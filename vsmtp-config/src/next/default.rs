#![allow(clippy::module_name_repetitions)]

use vsmtp_common::{code::SMTPReplyCode, collection};

pub fn default_smtp_codes() -> std::collections::BTreeMap<SMTPReplyCode, String> {
    let codes: std::collections::BTreeMap<SMTPReplyCode, &'static str> = collection! {
        SMTPReplyCode::Code214 => "214 joining us https://viridit.com/support\r\n",
        SMTPReplyCode::Code220 => "220 {domain} Service ready\r\n",
        SMTPReplyCode::Code221 => "221 Service closing transmission channel\r\n",
        SMTPReplyCode::Code250 => "250 Ok\r\n",
        SMTPReplyCode::Code250PlainEsmtp => "250-{domain}\r\n250-8BITMIME\r\n250-SMTPUTF8\r\n250 STARTTLS\r\n",
        SMTPReplyCode::Code250SecuredEsmtp => "250-{domain}\r\n250-8BITMIME\r\n250 SMTPUTF8\r\n",
        SMTPReplyCode::Code354 => "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
        SMTPReplyCode::Code451 => "451 Requested action aborted: local error in processing\r\n",
        SMTPReplyCode::Code451Timeout => "451 Timeout - closing connection.\r\n",
        SMTPReplyCode::Code451TooManyError => "451 Too many errors from the client\r\n",
        SMTPReplyCode::Code452 => "452 Requested action not taken: insufficient system storage\r\n",
        SMTPReplyCode::Code452TooManyRecipients => "452 Requested action not taken: to many recipients\r\n",
        SMTPReplyCode::Code454 => "454 TLS not available due to temporary reason\r\n",
        SMTPReplyCode::Code500 => "500 Syntax error command unrecognized\r\n",
        SMTPReplyCode::Code501 => "501 Syntax error in parameters or arguments\r\n",
        SMTPReplyCode::Code502unimplemented => "502 Command not implemented\r\n",
        SMTPReplyCode::Code503 => "503 Bad sequence of commands\r\n",
        SMTPReplyCode::Code504 => "504 Command parameter not implemented\r\n",
        SMTPReplyCode::Code530 => "530 Must issue a STARTTLS command first\r\n",
        SMTPReplyCode::Code554 => "554 permanent problems with the remote server\r\n",
        SMTPReplyCode::Code554tls => "554 Command refused due to lack of security\r\n",
        SMTPReplyCode::ConnectionMaxReached => "554 Cannot process connection, closing.\r\n",
    };

    assert!(
        <SMTPReplyCode as enum_iterator::IntoEnumIterator>::into_enum_iter()
            .all(|i| codes.contains_key(&i)),
        "default SMTPReplyCode are ill-formed "
    );

    codes
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect::<_>()
}
