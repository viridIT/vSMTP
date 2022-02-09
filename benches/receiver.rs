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
use std::collections::HashSet;

use criterion::{
    criterion_group, criterion_main, measurement::WallTime, Bencher, BenchmarkId, Criterion,
};
use vsmtp::{
    config::server_config::ServerConfig,
    mime::mail::BodyType,
    receiver::test_helpers::test_receiver,
    resolver::Resolver,
    rules::address::Address,
    smtp::mail::{Body, MailContext},
};

#[derive(Clone)]
struct DefaultResolverTest;

#[async_trait::async_trait]
impl Resolver for DefaultResolverTest {
    async fn deliver(&mut self, _: &ServerConfig, _: &MailContext) -> anyhow::Result<()> {
        Ok(())
    }
}

fn get_test_config() -> std::sync::Arc<ServerConfig> {
    std::sync::Arc::new(
        ServerConfig::builder()
            .with_rfc_port("bench.server.com")
            .without_log()
            .without_smtps()
            .with_default_smtp()
            .with_delivery("./tmp/bench", vsmtp::collection! {})
            .with_rules("./tmp/no_rules")
            .with_default_reply_codes()
            .build(),
    )
}

fn make_bench<R>(
    resolver: R,
    b: &mut Bencher<WallTime>,
    (input, output, config): &(&[u8], &[u8], std::sync::Arc<ServerConfig>),
) where
    R: Resolver + Clone + Send + Sync + 'static,
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
        impl Resolver for T {
            async fn deliver(&mut self, _: &ServerConfig, ctx: &MailContext) -> anyhow::Result<()> {
                assert_eq!(ctx.envelop.helo, "foobar");
                assert_eq!(ctx.envelop.mail_from.full(), "john@doe");
                assert_eq!(
                    ctx.envelop.rcpt,
                    HashSet::from([Address::new("aa@bb").unwrap()])
                );
                assert!(match &ctx.body {
                    Body::Parsed(mail) => mail.body == BodyType::Undefined,
                    _ => false,
                });

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
            |b, input| make_bench(T, b, input),
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
        |b, input| make_bench(DefaultResolverTest, b, input),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
