use criterion::{
    criterion_group, criterion_main, measurement::WallTime, Bencher, BenchmarkId, Criterion,
};
use vsmtp::{
    config::server_config::ServerConfig,
    mailprocessing::mail_receiver::{MailReceiver, StateSMTP},
    model::mail::MailContext,
    resolver::DataEndResolver,
    rules::address::Address,
    smtp::code::SMTPReplyCode,
    tests::Mock,
};

struct DefaultResolverTest;

#[async_trait::async_trait]
impl DataEndResolver for DefaultResolverTest {
    async fn on_data_end(_: &ServerConfig, _: &MailContext) -> (StateSMTP, SMTPReplyCode) {
        // after a successful exchange, the server is ready for a new RCPT
        (StateSMTP::MailFrom, SMTPReplyCode::Code250)
    }
}

fn get_test_config() -> ServerConfig {
    toml::from_str(include_str!("bench.config.toml")).expect("cannot parse config from toml")
}

fn make_bench<R: vsmtp::resolver::DataEndResolver>(
    b: &mut Bencher<WallTime>,
    (input, output, config): &(&[u8], &[u8], ServerConfig),
) {
    b.to_async(tokio::runtime::Runtime::new().unwrap())
        .iter(|| async {
            let mut config = config.clone();
            config.prepare();

            let mut receiver = MailReceiver::<R>::new(
                "0.0.0.0:0".parse().unwrap(),
                None,
                std::sync::Arc::new(config),
            );
            let mut write = Vec::new();
            let mock = Mock::new(input.to_vec(), &mut write);

            match receiver.receive_plain(mock).await {
                Ok(mut mock) => {
                    let _ = std::io::Write::flush(&mut mock);
                    assert_eq!(
                        std::str::from_utf8(&write),
                        std::str::from_utf8(&output.to_vec())
                    );
                }
                Err(e) => panic!("{}", e),
            }
        })
}

fn criterion_benchmark(c: &mut Criterion) {
    {
        struct T;

        #[async_trait::async_trait]
        impl DataEndResolver for T {
            async fn on_data_end(
                _: &ServerConfig,
                ctx: &MailContext,
            ) -> (StateSMTP, SMTPReplyCode) {
                assert_eq!(ctx.envelop.helo, "foobar");
                assert_eq!(ctx.envelop.mail_from.full(), "john@doe");
                assert_eq!(ctx.envelop.rcpt, vec![Address::new("aa@bb").unwrap()]);
                assert_eq!(ctx.body, "");

                (StateSMTP::MailFrom, SMTPReplyCode::Code250)
            }
        }

        c.bench_with_input(
            BenchmarkId::new("receiver", 0),
            &(
                [
                    "HELO foobar\r\n",
                    "MAIL FROM:<john@doe>\r\n",
                    "RCPT TO:<aa@bb>\r\n",
                    "DATA\r\n",
                    ".\r\n",
                    "QUIT\r\n",
                ]
                .concat()
                .as_bytes(),
                [
                    "220 bench.server.com Service ready\r\n",
                    "250 Ok\r\n",
                    "250 Ok\r\n",
                    "250 Ok\r\n",
                    "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                    "250 Ok\r\n",
                    "221 Service closing transmission channel\r\n",
                ]
                .concat()
                .as_bytes(),
                get_test_config(),
            ),
            |b, input| make_bench::<T>(b, input),
        );
    }

    c.bench_with_input(
        BenchmarkId::new("receiver", 1),
        &(
            ["foo\r\n"].concat().as_bytes(),
            [
                "220 bench.server.com Service ready\r\n",
                "501 Syntax error in parameters or arguments\r\n",
            ]
            .concat()
            .as_bytes(),
            get_test_config(),
        ),
        |b, input| make_bench::<DefaultResolverTest>(b, input),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
