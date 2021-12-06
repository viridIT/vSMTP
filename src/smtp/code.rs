/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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

// see https://datatracker.ietf.org/doc/html/rfc2034

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

#[derive(Debug, PartialEq)]
pub enum SMTPReplyCode {
    /// system status, or system help reply
    Code211,
    /// help message
    Code214,
    /// service ready
    Code220,
    /// service closing transmission channel
    Code221,
    /// requested mail action okay, completed
    Code250,
    /// ehlo message
    Code250PlainEsmtp,
    /// esmtp ehlo message
    Code250SecuredEsmtp,
    /// user not local; will forward
    Code251,
    /// cannot verify the user, but it will try to deliver the message anyway
    Code252,
    ///
    /// start mail input
    Code354,
    ///
    /// service not available, closing transmission channel
    Code421,
    /// requested mail action not taken: mailbox unavailable
    Code450,
    /// requested action aborted: local error in processing
    Code451,
    Code451Timeout,
    Code451TooManyError,
    /// requested action not taken: insufficient system storage
    Code452,
    // TLS not available due to temporary reason
    Code454,
    /// server unable to accommodate parameters
    Code455,
    ///
    /// syntax error, command unrecognized
    Code500,
    /// syntax error in parameters or arguments
    Code501,
    /// command not implemented
    Code502,
    /// bad sequence of commands
    Code503,
    /// command parameter is not implemented
    Code504,
    /// server does not accept mail
    Code521,
    /// encryption Needed
    Code523,

    /// 530 Must issue a STARTTLS command first
    /// NOTE:
    /// A SMTP server that is not publicly referenced may choose to require
    /// that the client perform a TLS negotiation before accepting any
    /// commands.  In this case, the server SHOULD return the reply code:
    Code530,
    /// requested action not taken: mailbox unavailable
    Code550,
    /// user not local; please try <forward-path>
    Code551,
    /// requested mail action aborted: exceeded storage allocation
    Code552,
    /// requested action not taken: mailbox name not allowed
    Code553,
    /// connection has been denied.
    Code554,
    /// transaction has failed
    Code554tls,
    Code555,
    /// domain does not accept mail
    Code556,
}

lazy_static::lazy_static! {
    static ref DOMAIN: String = {
        crate::config::get::<String>("domain")
                .expect("'domain' is a mandatory field in the config")
    };
    static ref CODE_220: String = {
        ["220 ", &DOMAIN, " Service ready\r\n"].concat()
    };
    static ref CODE_250_PLAIN_ESMTP: String = {
        ["250-", &DOMAIN, "\r\n", "250 STARTTLS\r\n"].concat()
    };
    static ref CODE_250_SECURED_ESMTP: String = {
        ["250 ", &DOMAIN, "\r\n"].concat()
    };
}

impl SMTPReplyCode {
    // TODO: make sure it is compliant
    // https://datatracker.ietf.org/doc/html/rfc5321#section-4.2
    pub fn as_str(&self) -> &'static str {
        match self {
            SMTPReplyCode::Code214 => "214 joining us https://viridit.com/support\r\n",
            SMTPReplyCode::Code220 => &CODE_220,
            SMTPReplyCode::Code221 => "221 Service closing transmission channel\r\n",
            SMTPReplyCode::Code250 => "250 Ok\r\n",
            SMTPReplyCode::Code250PlainEsmtp => &CODE_250_PLAIN_ESMTP,
            SMTPReplyCode::Code250SecuredEsmtp => &CODE_250_SECURED_ESMTP,
            //
            SMTPReplyCode::Code354 => "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            //
            SMTPReplyCode::Code451 => "451 Requested action aborted: local error in processing\r\n",
            SMTPReplyCode::Code451Timeout => "451 Timeout - closing connection.\r\n",
            SMTPReplyCode::Code451TooManyError => "451 Too many errors from the client\r\n",
            SMTPReplyCode::Code454 => "454 TLS not available due to temporary reason\r\n",
            //
            SMTPReplyCode::Code500 => "500 Syntax error, command unrecognized\r\n",
            SMTPReplyCode::Code501 => "501 Syntax error in parameters or arguments\r\n",
            SMTPReplyCode::Code502 => "502 Command not implemented\r\n",
            SMTPReplyCode::Code503 => "503 Bad sequence of commands\r\n",
            SMTPReplyCode::Code530 => "530 Must issue a STARTTLS command first\r\n",
            SMTPReplyCode::Code554 => "554 permanent problems with the remote server\r\n",
            SMTPReplyCode::Code554tls => "554 Command refused due to lack of security\r\n",

            _ => unimplemented!(),
        }
    }

    pub(crate) fn is_error(&self) -> bool {
        match self {
            SMTPReplyCode::Code214
            | SMTPReplyCode::Code220
            | SMTPReplyCode::Code221
            | SMTPReplyCode::Code250
            | SMTPReplyCode::Code250PlainEsmtp
            | SMTPReplyCode::Code250SecuredEsmtp
            | SMTPReplyCode::Code354 => false,
            //
            SMTPReplyCode::Code451Timeout
            | SMTPReplyCode::Code451
            | SMTPReplyCode::Code454
            | SMTPReplyCode::Code500
            | SMTPReplyCode::Code501
            | SMTPReplyCode::Code502
            | SMTPReplyCode::Code503
            | SMTPReplyCode::Code530
            | SMTPReplyCode::Code554
            | SMTPReplyCode::Code554tls => true,
            //
            _ => unimplemented!(),
        }
    }
}
