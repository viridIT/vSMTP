/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use criterion::{
    criterion_group, criterion_main, measurement::WallTime, Bencher, BenchmarkId, Criterion,
};
use trust_dns_resolver::TokioAsyncResolver;
use vsmtp_common::{address::Address, mail_context::MessageMetadata, rcpt::Rcpt};
use vsmtp_config::Config;
use vsmtp_delivery::transport::Transport;
use vsmtp_server::receiver::test_helpers::test_receiver;

#[derive(Clone)]
struct DefaultResolverTest;

#[async_trait::async_trait]
impl Transport for DefaultResolverTest {
    async fn deliver(
        &mut self,
        _: &Config,
        _: &TokioAsyncResolver,
        _: &MessageMetadata,
        _: &Address,
        _: &mut [Rcpt],
        _: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

fn get_test_config() -> std::sync::Arc<Config> {
    std::sync::Arc::new(
        Config::builder()
            .with_version_str("<1.0.0")
            .unwrap()
            .with_server_name("testserver.com")
            .with_user_group_and_default_system("root", "root")
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_spool_dir_and_default_queues("./tmp/delivery")
            .without_tls_support()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_default_smtp_codes()
            .with_default_app()
            .with_vsl("./benches/main.vsl")
            .with_default_app_logs()
            .without_services()
            .with_system_dns()
            .without_delivery_targets()
            .validate()
            .unwrap(),
    )
}

fn make_bench<R>(
    resolver: R,
    b: &mut Bencher<WallTime>,
    (input, output, config): &(&[u8], &[u8], std::sync::Arc<Config>),
) where
    R: Transport + Clone + Send + Sync + 'static,
{
    b.to_async(tokio::runtime::Runtime::new().unwrap())
        .iter(|| async {
            let _ = test_receiver(
                "127.0.0.1:0",
                resolver.clone(),
                input,
                output,
                config.clone(),
            )
            .await;
        })
}

fn criterion_benchmark(c: &mut Criterion) {
    {
        #[derive(Clone)]
        struct T;

        #[async_trait::async_trait]
        impl Transport for T {
            async fn deliver(
                &mut self,
                _: &Config,
                _: &TokioAsyncResolver,
                _: &MessageMetadata,
                from: &Address,
                rcpt: &mut [Rcpt],
                body: &str,
            ) -> anyhow::Result<()> {
                // assert_eq!(ctx.envelop.helo, "foobar");
                assert_eq!(from.full(), "john@doe");
                assert_eq!(
                    rcpt,
                    vec![Address::try_from("aa@bb".to_string()).unwrap().into()]
                );
                assert!(body.is_empty());

                Ok(())
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
                    "220 testserver.com Service ready\r\n",
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
            |b, input| make_bench(T, b, input),
        );
    }

    c.bench_with_input(
        BenchmarkId::new("receiver", 1),
        &(
            ["foo\r\n"].concat().as_bytes(),
            [
                "220 testserver.com Service ready\r\n",
                "501 Syntax error in parameters or arguments\r\n",
            ]
            .concat()
            .as_bytes(),
            get_test_config(),
        ),
        |b, input| make_bench(DefaultResolverTest, b, input),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
