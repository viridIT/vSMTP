/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
/// 2yz  Positive Completion reply
/// 3yz  Positive Intermediate reply
/// 4yz  Transient Negative Completion reply
/// 5yz  Permanent Negative Completion reply

/// x0z  Syntax: These replies refer to syntax errors, syntactically
/// correct commands that do not fit any functional category, and
/// unimplemented or superfluous commands.
///
/// x1z  Information: These are replies to requests for information, such
/// as status or help.
///
/// x2z  Connections: These are replies referring to the transmission
/// channel.
///
/// x3z  Unspecified.
/// x4z  Unspecified.
///
/// x5z  Mail system: These replies indicate the status of the receiver
/// mail system vis-a-vis the requested transfer or other mail system
/// action.
#[allow(clippy::module_name_repetitions)]
#[derive(
    Debug,
    Ord,
    PartialOrd,
    PartialEq,
    Eq,
    Hash,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumIter,
)]
#[serde(untagged)]
#[serde(into = "String")]
#[serde(try_from = "String")]
pub enum SMTPReplyCode {
    /// system status, or system help reply
    // Code211,
    /// help message
    Help,
    /// service ready
    Greetings,
    /// service closing transmission channel
    Code221,
    /// requested mail action okay, completed
    Code250,
    /// ehlo message
    Code250PlainEsmtp,
    /// esmtp ehlo message
    Code250SecuredEsmtp,
    /// user not local; will forward
    // Code251,
    /// cannot verify the user, but it will try to deliver the message anyway
    // Code252,
    ///
    /// start mail input
    Code354,
    ///
    /// service not available, closing transmission channel
    // Code421,
    /// requested mail action not taken: mailbox unavailable
    // Code450,
    /// requested action aborted: local error in processing
    Code451,
    ///
    Code451Timeout,
    ///
    Code451TooManyError,
    /// requested action not taken: insufficient system storage
    Code452,
    ///
    Code452TooManyRecipients,
    /// TLS not available due to temporary reason
    Code454,
    /// server unable to accommodate parameters
    // Code455,
    ///
    /// syntax error, command unrecognized
    Code500,
    /// syntax error in parameters or arguments
    Code501,
    /// command not implemented
    Code502unimplemented,
    /// bad sequence of commands
    BadSequence,
    /// command parameter is not implemented
    Code504,
    /// server does not accept mail
    // Code521,
    /// encryption Needed
    // Code523,

    /// 530 Must issue a STARTTLS command first
    Code530,
    /// requested action not taken: mailbox unavailable
    // Code550,
    /// user not local; please try <forward-path>
    // Code551,
    /// requested mail action aborted: exceeded storage allocation
    // Code552,
    /// requested action not taken: mailbox name not allowed
    // Code553,
    /// connection has been denied.
    Code554,
    /// transaction has failed
    Code554tls,
    /// 554 5.5.1 Error: TLS already active
    TlsAlreadyUnderTls,
    // Code555,
    // domain does not accept mail
    // Code556,
    /// 554
    ConnectionMaxReached,

    /// 504 5.5.4
    AuthMechanismNotSupported,
    /// 235 2.7.0
    AuthSucceeded,
    /// 538 5.7.11 Encryption required for requested authentication mechanism
    AuthMechanismMustBeEncrypted,
    /// 501 5.7.0 Client must not start with this mechanism
    AuthClientMustNotStart,
    /// 501 5.5.2
    AuthErrorDecode64,
    /// 535 5.7.8 Authentication credentials invalid
    AuthInvalidCredentials,
    /// 501
    AuthClientCanceled,
    /// 530 5.7.0 Authentication required
    AuthRequired,
    /// A custom reply code defined using vsl.
    Custom(String),
}

impl SMTPReplyCode {
    /// Is the code considered as an error
    #[must_use]
    pub const fn is_error(&self) -> bool {
        match self {
            Self::Help
            | Self::Greetings
            | Self::Code221
            | Self::Code250
            | Self::Code250PlainEsmtp
            | Self::Code250SecuredEsmtp
            | Self::Code354
            | Self::AuthSucceeded
            | Self::Custom(..) => false,
            Self::Code451Timeout
            | Self::Code451
            | Self::Code452
            | Self::Code452TooManyRecipients
            | Self::Code454
            | Self::Code500
            | Self::Code501
            | Self::Code502unimplemented
            | Self::BadSequence
            | Self::Code530
            | Self::Code554
            | Self::Code554tls
            | Self::ConnectionMaxReached
            | Self::Code451TooManyError
            | Self::Code504
            | Self::AuthMechanismNotSupported
            | Self::AuthMechanismMustBeEncrypted
            | Self::AuthClientMustNotStart
            | Self::AuthErrorDecode64
            | Self::AuthInvalidCredentials
            | Self::AuthClientCanceled
            | Self::AuthRequired
            | Self::TlsAlreadyUnderTls => true,
        }
    }
}

impl std::fmt::Display for SMTPReplyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Help => "Help",
            Self::Greetings => "Greetings",
            Self::Code221 => "Code221",
            Self::Code250 => "Code250",
            Self::Code250PlainEsmtp => "Code250PlainEsmtp",
            Self::Code250SecuredEsmtp => "Code250SecuredEsmtp",
            Self::Code354 => "Code354",
            Self::Code451 => "Code451",
            Self::Code451Timeout => "Code451Timeout",
            Self::Code451TooManyError => "Code451TooManyError",
            Self::Code452 => "Code452",
            Self::Code452TooManyRecipients => "Code452TooManyRecipients",
            Self::Code454 => "Code454",
            Self::Code500 => "Code500",
            Self::Code501 => "Code501",
            Self::Code502unimplemented => "Code502unimplemented",
            Self::BadSequence => "BadSequence",
            Self::Code504 => "Code504",
            Self::Code530 => "Code530",
            Self::Code554 => "Code554",
            Self::Code554tls => "Code554tls",
            Self::ConnectionMaxReached => "ConnectionMaxReached",
            Self::AuthMechanismNotSupported => "AuthMechanismNotSupported",
            Self::AuthSucceeded => "AuthSucceeded",
            Self::AuthMechanismMustBeEncrypted => "AuthMechanismMustBeEncrypted",
            Self::AuthClientMustNotStart => "AuthClientMustNotStart",
            Self::AuthErrorDecode64 => "AuthErrorDecode64",
            Self::AuthInvalidCredentials => "AuthInvalidCredentials",
            Self::AuthClientCanceled => "AuthClientCanceled",
            Self::AuthRequired => "AuthRequired",
            Self::TlsAlreadyUnderTls => "TlsAlreadyUnderTls",
            Self::Custom(_) => "Custom",
        })
    }
}

impl From<SMTPReplyCode> for String {
    fn from(code: SMTPReplyCode) -> Self {
        format!("{}", code)
    }
}

impl std::str::FromStr for SMTPReplyCode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Help" => Ok(Self::Help),
            "Greetings" => Ok(Self::Greetings),
            "Code221" => Ok(Self::Code221),
            "Code250" => Ok(Self::Code250),
            "Code250PlainEsmtp" => Ok(Self::Code250PlainEsmtp),
            "Code250SecuredEsmtp" => Ok(Self::Code250SecuredEsmtp),
            "Code354" => Ok(Self::Code354),
            "Code451" => Ok(Self::Code451),
            "Code451Timeout" => Ok(Self::Code451Timeout),
            "Code451TooManyError" => Ok(Self::Code451TooManyError),
            "Code452" => Ok(Self::Code452),
            "Code452TooManyRecipients" => Ok(Self::Code452TooManyRecipients),
            "Code454" => Ok(Self::Code454),
            "Code500" => Ok(Self::Code500),
            "Code501" => Ok(Self::Code501),
            "Code502unimplemented" => Ok(Self::Code502unimplemented),
            "BadSequence" => Ok(Self::BadSequence),
            "Code504" => Ok(Self::Code504),
            "Code530" => Ok(Self::Code530),
            "Code554" => Ok(Self::Code554),
            "Code554tls" => Ok(Self::Code554tls),
            "ConnectionMaxReached" => Ok(Self::ConnectionMaxReached),
            "AuthMechanismNotSupported" => Ok(Self::AuthMechanismNotSupported),
            "AuthSucceeded" => Ok(Self::AuthSucceeded),
            "AuthMechanismMustBeEncrypted" => Ok(Self::AuthMechanismMustBeEncrypted),
            "AuthClientMustNotStart" => Ok(Self::AuthClientMustNotStart),
            "AuthErrorDecode64" => Ok(Self::AuthErrorDecode64),
            "AuthInvalidCredentials" => Ok(Self::AuthInvalidCredentials),
            "AuthClientCanceled" => Ok(Self::AuthClientCanceled),
            "AuthRequired" => Ok(Self::AuthRequired),
            "TlsAlreadyUnderTls" => Ok(Self::TlsAlreadyUnderTls),
            "Custom" => Ok(Self::Custom(String::default())),
            _ => Err(anyhow::anyhow!("not a valid SMTPReplyCode: '{}'", s)),
        }
    }
}

impl TryFrom<String> for SMTPReplyCode {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        <Self as std::str::FromStr>::from_str(&value)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::SMTPReplyCode;

    #[test]
    fn error() {
        assert_eq!(
            format!("{}", SMTPReplyCode::from_str("foobar").unwrap_err()),
            "not a valid SMTPReplyCode: 'foobar'"
        );
    }

    #[test]
    fn same() {
        for s in <SMTPReplyCode as strum::IntoEnumIterator>::iter() {
            println!("{:?} error={}", s, s.is_error());
            assert_eq!(SMTPReplyCode::from_str(&format!("{}", s)).unwrap(), s);
            assert_eq!(String::try_from(s.clone()).unwrap(), format!("{}", s));
        }
    }
}
