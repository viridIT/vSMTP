use std::collections::HashSet;

use criterion::{
    criterion_group, criterion_main, measurement::WallTime, Bencher, BenchmarkId, Criterion,
};
use vsmtp::{
    config::server_config::ServerConfig, model::mail::MailContext, resolver::DataEndResolver,
    rules::address::Address, smtp::code::SMTPReplyCode, test_helpers::test_receiver,
};

struct DefaultResolverTest;

#[async_trait::async_trait]
impl DataEndResolver for DefaultResolverTest {
    async fn on_data_end(
        &mut self,
        _: &ServerConfig,
        _: &MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        Ok(SMTPReplyCode::Code250)
    }
}

fn get_test_config() -> ServerConfig {
    toml::from_str(include_str!("bench.config.toml")).expect("cannot parse config from toml")
}

fn make_bench<R: vsmtp::resolver::DataEndResolver>(
    resolver: std::sync::Arc<tokio::sync::Mutex<R>>,
    b: &mut Bencher<WallTime>,
    (input, output, config): &(&[u8], &[u8], ServerConfig),
) {
    b.to_async(tokio::runtime::Runtime::new().unwrap())
        .iter(|| async {
            let _ = test_receiver(resolver.clone(), input, output, config.clone()).await;
        })
}

fn criterion_benchmark(c: &mut Criterion) {
    {
        struct T;

        #[async_trait::async_trait]
        impl DataEndResolver for T {
            async fn on_data_end(
                &mut self,
                _: &ServerConfig,
                ctx: &MailContext,
            ) -> Result<SMTPReplyCode, std::io::Error> {
                assert_eq!(ctx.envelop.helo, "foobar");
                assert_eq!(ctx.envelop.mail_from.full(), "john@doe");
                assert_eq!(
                    ctx.envelop.rcpt,
                    HashSet::from([Address::new("aa@bb").unwrap()])
                );
                assert_eq!(ctx.body, "");

                Ok(SMTPReplyCode::Code250)
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
            |b, input| make_bench(std::sync::Arc::new(tokio::sync::Mutex::new(T {})), b, input),
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
        |b, input| {
            make_bench(
                std::sync::Arc::new(tokio::sync::Mutex::new(DefaultResolverTest {})),
                b,
                input,
            )
        },
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
