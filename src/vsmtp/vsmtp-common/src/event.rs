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
use crate::{mechanism::Mechanism, Address, CodeID};

/// See "SMTP Service Extension for 8-bit MIME Transport"
/// https://datatracker.ietf.org/doc/html/rfc6152
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MimeBodyType {
    ///
    SevenBit,
    ///
    EightBitMime,
    // Binary, // TODO: https://datatracker.ietf.org/doc/html/rfc3030
}

impl std::str::FromStr for MimeBodyType {
    type Err = CodeID;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "7BIT" => Ok(Self::SevenBit),
            "8BITMIME" => Ok(Self::EightBitMime),
            _ => Err(CodeID::SyntaxErrorParams),
        }
    }
}

/// Command SMTPs sent and received by servers and clients
/// See "Simple Mail Transfer Protocol"
/// https://datatracker.ietf.org/doc/html/rfc5321
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Event {
    /// Used to identify the SMTP client to the SMTP server.
    /// Syntax = `"HELO" SP ( Domain / address-literal ) CRLF`
    HeloCmd(String),
    /// Used to identify the SMTP client to the SMTP server and request smtp extensions.
    /// Syntax = `"EHLO" SP ( Domain / address-literal ) CRLF`
    EhloCmd(String),
    /// This command is used to initiate a mail transaction in which the mail
    /// data is delivered to an SMTP server that may, in turn, deliver it to
    /// one or more mailboxes or pass it on to another system (possibly using
    /// SMTP).
    /// Syntax = `"MAIL FROM:" Reverse-path [SP Mail-parameters] CRLF`
    ///
    /// 3rd argument is an xtext of the identity of the submitter,
    /// "<>" meaning not enough unknown or insufficiently authenticated
    MailCmd(Option<Address>, Option<MimeBodyType>, Option<String>),
    /// This command is used to identify an individual recipient of the mail
    /// data; multiple recipients are specified by multiple uses of this
    /// command.
    /// Syntax = `"RCPT TO:" ( "<Postmaster@" Domain ">" / "<Postmaster>" /
    /// Forward-path ) [SP Rcpt-parameters] CRLF`
    RcptCmd(Address),
    /// This command causes the mail data to be appended to the mail data
    /// buffer.
    /// Syntax = `"DATA" CRLF`
    DataCmd,
    /// Lines ended by CRLF sent between [Event::DataCmd] and [Event::DataEnd]
    DataLine(String),
    /// The mail data are terminated by a line containing only a period. This
    /// is the end of mail data indication.
    /// Syntax = `"." CRLF`
    DataEnd,
    /// "RSET\r\n"
    /// This command specifies that the current mail transaction will be
    /// aborted. Any stored sender, recipients, and mail data MUST be
    /// discarded, and all buffers and state tables cleared.
    /// Syntax = `"RSET" CRLF`
    RsetCmd,
    /// This command asks the receiver to confirm that the argument
    /// identifies a user or mailbox.
    /// Syntax = `"VRFY" SP String CRLF`
    VrfyCmd(String),
    /// This command asks the receiver to confirm that the argument
    /// identifies a mailing list, and if so, to return the membership of
    /// that list.
    /// Syntax = `"EXPN" SP String CRLF`
    ExpnCmd(String),
    /// This command causes the server to send helpful information to the
    /// client. The command MAY take an argument (e.g., any command name)
    /// and return more specific information as a response.
    /// Syntax = `"HELP" [ SP String ] CRLF`
    HelpCmd(Option<String>),
    /// This command does not affect any parameters or previously entered
    /// commands.
    /// Syntax = `"NOOP" [ SP String ] CRLF`
    NoopCmd,
    /// This command specifies that the receiver MUST send a "221 OK" reply,
    /// and then close the transmission channel.
    /// Syntax = `"QUIT" CRLF`
    QuitCmd,

    /// See "Transport Layer Security"
    /// https://datatracker.ietf.org/doc/html/rfc3207
    /// Syntax = `"STARTTLS" CRLF`
    StartTls,
    //
    // TODO:
    // PrivCmd,
    //
    /// Authentication with SASL protocol
    /// https://datatracker.ietf.org/doc/html/rfc4954
    /// Syntax = `"AUTH" mechanism [initial-response] CRLF`
    Auth(Mechanism, Option<Vec<u8>>),
    //
    // Authenticated TURN for On-Demand Mail Relay // https://datatracker.ietf.org/doc/html/rfc2645
    // Chunking // https://datatracker.ietf.org/doc/html/rfc3030
    // Delivery status notification // https://datatracker.ietf.org/doc/html/rfc3461
    // https://en.wikipedia.org/wiki/Variable_envelope_return_path
    // Extended version of remote message queue starting command TURN
    // https://datatracker.ietf.org/doc/html/rfc1985
    // Command pipelining // https://datatracker.ietf.org/doc/html/rfc2920
    // Message size declaration // https://datatracker.ietf.org/doc/html/rfc1870
}

impl Event {
    /// Create a valid SMTP command (or event) from a string OR return a SMTP error code
    /// See https://datatracker.ietf.org/doc/html/rfc5321#section-4.1
    ///
    /// # Errors
    pub fn parse_cmd(input: &str) -> Result<Self, CodeID> {
        // 88 = 80 - "\r\n".len() + (SMTPUTF8 ? 10 : 0)
        if input.len() > 88 || input.is_empty() {
            return Err(CodeID::UnrecognizedCommand);
        }

        let words = input
            .split_whitespace()
            // .inspect(|x| log::trace!(target: RECEIVER, "word:{}", x))
            .collect::<Vec<&str>>();

        let mut smtp_args = words.iter();
        let smtp_verb = match smtp_args.next() {
            // TODO: verify rfc about that..
            // NOTE: if the first word is not the beginning of the input (whitespace before)
            Some(fist_word) if &input[..fist_word.len()] != *fist_word => {
                return Err(CodeID::SyntaxErrorParams);
            }
            Some(smtp_verb) => smtp_verb,
            None => return Err(CodeID::UnrecognizedCommand),
        };

        match (
            smtp_verb.to_ascii_uppercase().as_str(),
            smtp_args.as_slice(),
        ) {
            ("HELO", args) => Self::parse_arg_helo(args),
            ("EHLO", args) => Self::parse_arg_ehlo(args),
            ("MAIL", args) => Self::parse_arg_mail_from(args),
            ("RCPT", args) => Self::parse_arg_rcpt_to(args),

            ("VRFY", [user_or_mailbox] | [user_or_mailbox, "SMTPUTF8"]) => {
                Ok(Self::VrfyCmd((*user_or_mailbox).to_string()))
            }
            ("EXPN", [mailing_list] | [mailing_list, "SMTPUTF8"]) => {
                Ok(Self::ExpnCmd((*mailing_list).to_string()))
            }

            ("HELP", []) => Ok(Self::HelpCmd(None)),
            ("HELP", [help_value]) => Ok(Self::HelpCmd(Some((*help_value).to_string()))),

            ("DATA", []) => Ok(Self::DataCmd),
            ("QUIT", []) => Ok(Self::QuitCmd),
            ("RSET", []) => Ok(Self::RsetCmd),
            ("NOOP", [..]) => Ok(Self::NoopCmd),

            ("STARTTLS", []) => Ok(Self::StartTls),
            ("AUTH", [mechanism]) => Self::parse_arg_auth(mechanism, None),
            ("AUTH", [mechanism, initial_response]) => {
                Self::parse_arg_auth(mechanism, Some(initial_response))
            }

            _ => Err(CodeID::SyntaxErrorParams),
        }
    }

    fn parse_domain_or_address_literal(args: &[&str]) -> anyhow::Result<String> {
        match args {
            [ip] if ip.starts_with('[') && ip.ends_with(']') => Ok(ip[1..ip.len() - 1]
                .parse::<std::net::IpAddr>()
                .map_err(|e| anyhow::anyhow!(e))?
                .to_string()),
            [domain] => Ok(addr::parse_domain_name(domain)
                .map_err(|e| anyhow::anyhow!(e.input().to_string()))?
                .to_string()),
            _ => anyhow::bail!("no domain or ip found in arguments"),
        }
    }

    fn parse_arg_helo(args: &[&str]) -> Result<Self, CodeID> {
        match Self::parse_domain_or_address_literal(args) {
            Ok(out) => Ok(Self::HeloCmd(out)),
            Err(_) => Err(CodeID::SyntaxErrorParams),
        }
    }

    fn parse_arg_ehlo(args: &[&str]) -> Result<Self, CodeID> {
        match Self::parse_domain_or_address_literal(args) {
            Ok(out) => Ok(Self::EhloCmd(out)),
            Err(_) => Err(CodeID::SyntaxErrorParams),
        }
    }

    pub(super) fn from_path(input: &str, may_be_empty: bool) -> Result<String, CodeID> {
        if input.starts_with('<') && input.ends_with('>') {
            match &input[1..input.len() - 1] {
                "" if may_be_empty => Ok("".to_string()),
                // TODO: should accept and ignore A-d-l ("source route")
                // https://datatracker.ietf.org/doc/html/rfc5321#section-4.1.2
                mailbox => match addr::parse_email_address(mailbox) {
                    Ok(mailbox) => Ok(mailbox.to_string()),
                    Err(_) => Err(CodeID::SyntaxErrorParams),
                },
            }
        } else {
            Err(CodeID::SyntaxErrorParams)
        }
    }

    fn parse_arg_mail_from(args: &[&str]) -> Result<Self, CodeID> {
        fn parse_esmtp_args(path: String, args: &[&str]) -> Result<Event, CodeID> {
            let mut bitmime = None;
            let mut auth_mailbox = None;

            for arg in args {
                if let Some(raw) = arg.strip_prefix("BODY=") {
                    if bitmime.is_none() {
                        bitmime = Some(<MimeBodyType as std::str::FromStr>::from_str(raw)?);
                    } else {
                        return Err(CodeID::SyntaxErrorParams);
                    }
                } else if *arg == "SMTPUTF8" {
                    // TODO: ?
                    // do we want to set a flag in the envelope to force utf8 in the deliver/relay ?
                } else if let Some(mailbox) = arg.strip_prefix("AUTH=") {
                    if auth_mailbox.is_none() {
                        auth_mailbox = Some(mailbox.to_string());
                    } else {
                        return Err(CodeID::SyntaxErrorParams);
                    }
                } else {
                    return Err(CodeID::ParameterUnimplemented);
                }
            }

            Ok(Event::MailCmd(
                if path.is_empty() {
                    None
                } else {
                    Some(Address::try_from(path).map_err(|_| CodeID::SyntaxErrorParams)?)
                },
                bitmime,
                auth_mailbox,
            ))
        }

        match args {
            // note: separated word (can return a warning)
            [from, reverse_path, ..] if from.to_ascii_uppercase() == "FROM:" => {
                parse_esmtp_args(Self::from_path(reverse_path, true)?, &args[2..])
            }
            [from_and_reverse_path, ..] => match from_and_reverse_path
                .to_ascii_uppercase()
                .strip_prefix("FROM:")
            {
                Some("") | None => Err(CodeID::SyntaxErrorParams),
                Some(_) => parse_esmtp_args(
                    Self::from_path(&from_and_reverse_path["FROM:".len()..], true)?,
                    &args[1..],
                ),
            },
            _ => Err(CodeID::SyntaxErrorParams),
        }
    }

    fn parse_arg_rcpt_to(args: &[&str]) -> Result<Self, CodeID> {
        // TODO: https://datatracker.ietf.org/doc/html/rfc5321#section-4.1.1.3
        // Syntax = "RCPT TO:" ( "<Postmaster@" Domain ">" / "<Postmaster>" /
        //         Forward-path ) [SP Rcpt-parameters] CRLF
        // Note that, in a departure from the usual rules for
        // local-parts, the "Postmaster" string shown above is
        // treated as case-insensitive.

        // TODO: parse "<Postmaster@" Domain ">" / "<Postmaster>"

        #[allow(clippy::missing_const_for_fn)]
        fn parse_esmtp_args(path: String, args: &[&str]) -> Result<Event, CodeID> {
            if args.is_empty() {
                Ok(Event::RcptCmd(
                    Address::try_from(path).map_err(|_| CodeID::SyntaxErrorParams)?,
                ))
            } else {
                Err(CodeID::ParameterUnimplemented)
            }
        }

        match args {
            // NOTE: separated word (can return a warning)
            [to, forward_path, ..] if to.to_ascii_uppercase() == "TO:" => {
                parse_esmtp_args(Self::from_path(forward_path, false)?, &args[2..])
            }
            [to_and_forward_path, ..] => {
                match to_and_forward_path.to_ascii_uppercase().strip_prefix("TO:") {
                    Some("") | None => Err(CodeID::SyntaxErrorParams),
                    Some(_) => parse_esmtp_args(
                        Self::from_path(&to_and_forward_path["TO:".len()..], false)?,
                        &args[1..],
                    ),
                }
            }
            _ => Err(CodeID::SyntaxErrorParams),
        }
    }

    fn parse_arg_auth(mechanism: &str, initial_response: Option<&str>) -> Result<Self, CodeID> {
        Ok(Self::Auth(
            <Mechanism as std::str::FromStr>::from_str(mechanism)
                .map_err(|_| CodeID::AuthMechNotSupported)?,
            initial_response.map(|s| s.as_bytes().to_vec()),
        ))
    }

    /// Parse a smtp input receive between DATA and <CRLF>.<CRLF> (DATA END)
    /// and handle dot-stuffing
    ///
    /// # Errors
    ///
    /// * input length is too long (> 998)
    pub fn parse_data(input: &str) -> Result<Self, CodeID> {
        match input {
            "." => Ok(Self::DataEnd),
            too_long if too_long.len() > 998 => Err(CodeID::UnrecognizedCommand),
            dot_string if dot_string.starts_with('.') => {
                // https://www.rfc-editor.org/rfc/rfc5321#section-4.5.2
                Ok(Self::DataLine(dot_string[1..].to_string()))
            }
            _ => Ok(Self::DataLine(input.to_string())),
        }
    }
}
