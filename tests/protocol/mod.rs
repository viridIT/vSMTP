#[cfg(test)]
mod tests {
    use std::io::Write;

    use v_smtp::{
        mailprocessing::mail_receiver::{MailReceiver, State},
        model::mail::MailContext,
        resolver::DataEndResolver,
        server::TlsSecurityLevel,
        smtp::code::SMTPReplyCode,
        tests::Mock,
    };

    // see https://datatracker.ietf.org/doc/html/rfc5321#section-4.3.2

    struct DataEndResolverTest;

    #[async_trait::async_trait]
    impl DataEndResolver for DataEndResolverTest {
        async fn on_data_end(_: &MailContext) -> (State, SMTPReplyCode) {
            (State::Helo, SMTPReplyCode::Code250)
        }
    }

    async fn make_test(smtp_input: &[u8], expected_output: &[u8]) {
        let mut receiver = MailReceiver::<DataEndResolverTest>::new(
            "0.0.0.0:0".parse().unwrap(),
            // std::time::Duration::from_millis(1_000),
            None,
            TlsSecurityLevel::May,
        );
        let mut write = Vec::new();
        let mock = Mock::new(smtp_input.to_vec(), &mut write);

        match receiver.receive_plain(mock).await {
            Ok(mut mock) => {
                let _ = mock.flush();
                assert_eq!(
                    std::str::from_utf8(&write),
                    std::str::from_utf8(&expected_output.to_vec())
                )
            }
            Err(e) => panic!("receiver produce an error {}", e),
        }
    }

    #[tokio::test]
    async fn test_receiver_1() {
        make_test(
            [
                "HELO foobar\r\n",
                "MAIL FROM:<jhon@doe>\r\n",
                "RCPT TO:<aa@bb>\r\n",
                "DATA\r\n",
                ".\r\n",
                "QUIT\r\n",
            ]
            .concat()
            .as_bytes(),
            [
                "220 Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n",
                "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                "250 Ok\r\n",
                "221 Service closing transmission channel\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_receiver_2() {
        make_test(
            ["foo\r\n"].concat().as_bytes(),
            [
                "220 Service ready\r\n",
                "501 Syntax error in parameters or arguments\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_receiver_3() {
        make_test(
            ["MAIL FROM:<jhon@doe>\r\n"].concat().as_bytes(),
            ["220 Service ready\r\n", "503 Bad sequence of commands\r\n"]
                .concat()
                .as_bytes(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_receiver_4() {
        make_test(
            ["RCPT TO:<jhon@doe>\r\n"].concat().as_bytes(),
            ["220 Service ready\r\n", "503 Bad sequence of commands\r\n"]
                .concat()
                .as_bytes(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_receiver_5() {
        make_test(
            ["HELO foo\r\n", "RCPT TO:<bar@foo>\r\n"]
                .concat()
                .as_bytes(),
            [
                "220 Service ready\r\n",
                "250 Ok\r\n",
                "503 Bad sequence of commands\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await;
    }

    #[tokio::test]
    async fn test_receiver_6() {
        make_test(
            ["HELO foobar\r\n", "QUIT\r\n"].concat().as_bytes(),
            [
                "220 Service ready\r\n",
                "250 Ok\r\n",
                "221 Service closing transmission channel\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await;
    }
}
