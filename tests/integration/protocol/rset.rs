#[cfg(test)]
mod tests {

    use std::collections::HashSet;

    use vsmtp::{
        config::server_config::ServerConfig,
        model::mail::{Body, MailContext},
        resolver::DataEndResolver,
        rules::address::Address,
        smtp::code::SMTPReplyCode,
        test_helpers::{test_receiver, DefaultResolverTest},
    };

    use crate::integration::protocol::get_test_config;

    #[tokio::test]
    async fn test_receiver_rset_1() {
        struct T;

        #[async_trait::async_trait]
        impl DataEndResolver for T {
            async fn on_data_end(
                &mut self,
                _: &ServerConfig,
                ctx: &MailContext,
            ) -> Result<SMTPReplyCode, std::io::Error> {
                assert_eq!(ctx.envelop.helo, "foo");
                assert_eq!(ctx.envelop.mail_from.full(), "a@b");
                assert_eq!(
                    ctx.envelop.rcpt,
                    HashSet::from([Address::new("b@c").unwrap()])
                );
                assert!(match &ctx.body {
                    Body::Parsed(body) => body.headers.is_empty(),
                    _ => false,
                });

                Ok(SMTPReplyCode::Code250)
            }
        }

        assert!(test_receiver::<T>(
            std::sync::Arc::new(tokio::sync::Mutex::new(T)),
            [
                "HELO foo\r\n",
                "RSET\r\n",
                "MAIL FROM:<a@b>\r\n",
                "RCPT TO:<b@c>\r\n",
                "DATA\r\n",
                "mail content wow\r\n",
                ".\r\n"
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                "250 Ok\r\n"
            ]
            .concat()
            .as_bytes(),
            get_test_config()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_rset_2() {
        assert!(test_receiver::<DefaultResolverTest>(
            std::sync::Arc::new(tokio::sync::Mutex::new(DefaultResolverTest)),
            [
                "HELO foo\r\n",
                "MAIL FROM:<a@b>\r\n",
                "RSET\r\n",
                "RCPT TO:<b@c>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "503 Bad sequence of commands\r\n",
            ]
            .concat()
            .as_bytes(),
            get_test_config()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_rset_3() {
        assert!(test_receiver::<DefaultResolverTest>(
            std::sync::Arc::new(tokio::sync::Mutex::new(DefaultResolverTest)),
            [
                "HELO foo\r\n",
                "MAIL FROM:<a@b>\r\n",
                "RSET\r\n",
                "HELO foo2\r\n",
                "RCPT TO:<b@c>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "503 Bad sequence of commands\r\n",
            ]
            .concat()
            .as_bytes(),
            get_test_config()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_rset_4() {
        struct T;

        #[async_trait::async_trait]
        impl DataEndResolver for T {
            async fn on_data_end(
                &mut self,
                _: &ServerConfig,
                ctx: &MailContext,
            ) -> Result<SMTPReplyCode, std::io::Error> {
                assert_eq!(ctx.envelop.helo, "foo2");
                assert_eq!(ctx.envelop.mail_from.full(), "d@e");
                assert_eq!(
                    ctx.envelop.rcpt,
                    HashSet::from([Address::new("b@c").unwrap()])
                );
                assert!(match &ctx.body {
                    Body::Parsed(body) => body.headers.is_empty(),
                    _ => false,
                });

                Ok(SMTPReplyCode::Code250)
            }
        }

        assert!(test_receiver::<T>(
            std::sync::Arc::new(tokio::sync::Mutex::new(T)),
            [
                "HELO foo\r\n",
                "MAIL FROM:<a@b>\r\n",
                "RSET\r\n",
                "HELO foo2\r\n",
                "MAIL FROM:<d@e>\r\n",
                "RCPT TO:<b@c>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
            ]
            .concat()
            .as_bytes(),
            get_test_config()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_rset_5() {
        struct T;

        #[async_trait::async_trait]
        impl DataEndResolver for T {
            async fn on_data_end(
                &mut self,
                _: &ServerConfig,
                ctx: &MailContext,
            ) -> Result<SMTPReplyCode, std::io::Error> {
                assert_eq!(ctx.envelop.helo, "foo");
                assert_eq!(ctx.envelop.mail_from.full(), "foo@foo");
                assert_eq!(
                    ctx.envelop.rcpt,
                    HashSet::from([Address::new("toto@bar").unwrap()])
                );
                assert!(match &ctx.body {
                    Body::Parsed(body) => body.headers.is_empty(),
                    _ => false,
                });

                Ok(SMTPReplyCode::Code250)
            }
        }

        assert!(test_receiver::<T>(
            std::sync::Arc::new(tokio::sync::Mutex::new(T)),
            [
                "HELO foo\r\n",
                "MAIL FROM:<foo@foo>\r\n",
                "RCPT TO:<toto@bar>\r\n",
                "RSET\r\n",
                "RCPT TO:<toto2@bar>\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "503 Bad sequence of commands\r\n",
            ]
            .concat()
            .as_bytes(),
            get_test_config()
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_receiver_rset_6() {
        struct T;

        #[async_trait::async_trait]
        impl DataEndResolver for T {
            async fn on_data_end(
                &mut self,
                _: &ServerConfig,
                ctx: &MailContext,
            ) -> Result<SMTPReplyCode, std::io::Error> {
                assert_eq!(ctx.envelop.helo, "foo");
                assert_eq!(ctx.envelop.mail_from.full(), "foo2@foo");
                assert_eq!(
                    ctx.envelop.rcpt,
                    HashSet::from([
                        Address::new("toto2@bar").unwrap(),
                        Address::new("toto3@bar").unwrap()
                    ])
                );
                assert!(match &ctx.body {
                    Body::Parsed(body) => body.headers.is_empty(),
                    _ => false,
                });

                Ok(SMTPReplyCode::Code250)
            }
        }

        assert!(test_receiver::<T>(
            std::sync::Arc::new(tokio::sync::Mutex::new(T)),
            [
                "HELO foo\r\n",
                "MAIL FROM:<foo@foo>\r\n",
                "RCPT TO:<toto@bar>\r\n",
                "RSET\r\n",
                "MAIL FROM:<foo2@foo>\r\n",
                "RCPT TO:<toto2@bar>\r\n",
                "RCPT TO:<toto3@bar>\r\n",
                "DATA\r\n",
                ".\r\n"
            ]
            .concat()
            .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                "250 Ok\r\n"
            ]
            .concat()
            .as_bytes(),
            get_test_config()
        )
        .await
        .is_ok());
    }
}
