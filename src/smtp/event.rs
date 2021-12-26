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
use super::code::SMTPReplyCode;

// see https://datatracker.ietf.org/doc/html/rfc6152
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum EightBitMimeBody {
    SevenBit,
    EightBitMime,
    // TODO: https://datatracker.ietf.org/doc/html/rfc3030
}

impl std::str::FromStr for EightBitMimeBody {
    type Err = SMTPReplyCode;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "7BIT" => Ok(EightBitMimeBody::SevenBit),
            "8BITMIME" => Ok(EightBitMimeBody::EightBitMime),
            _ => Err(SMTPReplyCode::Code501),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Event {
    // SMTP
    HeloCmd(String),
    EhloCmd(String),
    MailCmd(String, Option<EightBitMimeBody>),
    RcptCmd(String),
    DataCmd,
    DataLine(String),
    DataEnd,
    RsetCmd,
    VrfyCmd(String), // TODO:
    ExpnCmd(String), // TODO:
    HelpCmd(Option<String>),
    NoopCmd,
    QuitCmd,
    // PrivCmd, // TODO:

    // ESMTP

    // Authenticated TURN for On-Demand Mail Relay // https://datatracker.ietf.org/doc/html/rfc2645
    // ATRN,
    // Authenticated SMTP // https://datatracker.ietf.org/doc/html/rfc4954
    // AUTH,
    // Chunking // https://datatracker.ietf.org/doc/html/rfc3030
    // CHUNKING,
    // Delivery status notification // https://datatracker.ietf.org/doc/html/rfc3461
    // https://en.wikipedia.org/wiki/Variable_envelope_return_path
    // DSN,
    // Extended version of remote message queue starting command TURN
    // https://datatracker.ietf.org/doc/html/rfc1985
    // ETRN,
    // ?? HELP,       // Supply helpful information
    // Command pipelining // https://datatracker.ietf.org/doc/html/rfc2920
    // PIPELINING,
    // Message size declaration // https://datatracker.ietf.org/doc/html/rfc1870
    // SIZE,
    //  Transport Layer Security // https://datatracker.ietf.org/doc/html/rfc3207
    StartTls,
    // Allow UTF-8 encoding in mailbox names and header fields
    // https://datatracker.ietf.org/doc/html/rfc6531
    // SMTPUTF8,
    // UTF8SMTP,
    // https://datatracker.ietf.org/doc/html/rfc2034
    // EnhancedStatusCodes,
}

impl Event {
    /// Create a valid SMTP command (or event) from a string OR return a SMTP error code
    /// See https://datatracker.ietf.org/doc/html/rfc5321#section-4.1
    ///
    /// # Examples
    ///
    /// Just Errors
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd(""), Err(SMTPReplyCode::Code500));
    /// assert_eq!(
    ///     Event::parse_cmd(std::str::from_utf8(&vec![b'_'; 100][..]).unwrap()),
    ///     Err(SMTPReplyCode::Code500)
    /// );
    /// ```
    ///
    /// Helo Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("HELO foobar"), Ok(Event::HeloCmd("foobar".to_string())));
    /// assert_eq!(Event::parse_cmd("hElO   ibm.com  "), Ok(Event::HeloCmd("ibm.com".to_string())));
    /// assert_eq!(Event::parse_cmd("hElO  not\\a.valid\"domain"), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("hElO  "), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("   hElO  valid_domain"), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("HELO one two"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Ehlo Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("EHLO foobar"), Ok(Event::EhloCmd("foobar".to_string())));
    /// assert_eq!(Event::parse_cmd("EHLO   ibm.com  "), Ok(Event::EhloCmd("ibm.com".to_string())));
    /// assert_eq!(Event::parse_cmd("hElO  not\\a.valid\"domain"), Err(SMTPReplyCode::Code501));
    /// assert_eq!(
    ///     Event::parse_cmd("EHLO   [127.0.0.1]  "),
    ///     Ok(Event::EhloCmd("127.0.0.1".to_string()))
    /// );
    /// assert_eq!(
    ///     Event::parse_cmd("ehlo   [0011:2233:4455:6677:8899:aabb:ccdd:eeff]  "),
    ///     Ok(Event::EhloCmd("11:2233:4455:6677:8899:aabb:ccdd:eeff".to_string()))
    /// );
    /// assert_eq!(Event::parse_cmd("EHLO  "), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("   EHLO  valid_domain"), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("EHLO one two"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Mail from Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, event::EightBitMimeBody, code::SMTPReplyCode};
    ///
    /// assert_eq!(
    ///     Event::parse_cmd("Mail FROM:<valid@reverse.path.com>"),
    ///     Ok(Event::MailCmd("valid@reverse.path.com".to_string(), None))
    /// );
    /// assert_eq!(
    ///     Event::parse_cmd("Mail fRoM: <valid2@reverse.path.com>"),
    ///     Ok(Event::MailCmd("valid2@reverse.path.com".to_string(), None))
    /// );
    /// assert_eq!(Event::parse_cmd("MaIl From:   <>  "), Ok(Event::MailCmd("".to_string(), None)));
    /// // FIXME:
    /// // assert_eq!(
    /// //     Event::parse_cmd("MaIl From:   <local.part@[127.0.0.1]>  "),
    /// //     Ok(Event::MailCmd("local.part@[127.0.0.1]".to_string(), None))
    /// // );
    /// assert_eq!(
    ///     Event::parse_cmd("MaIl From:   <\"john..doe\"@example.org>  "),
    ///     Ok(Event::MailCmd("\"john..doe\"@example.org".to_string(), None))
    /// );
    /// assert_eq!(Event::parse_cmd("Mail  From:  "), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("Mail From ko"), Err(SMTPReplyCode::Code501));
    ///
    /// ```
    ///
    /// Mail from Command with 8bit-MIMEtransport extension
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, event::EightBitMimeBody, code::SMTPReplyCode};
    ///
    /// assert_eq!(
    ///     Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY=8BITMIME"),
    ///     Ok(Event::MailCmd("ned@ymir.claremont.edu".to_string(), Some(EightBitMimeBody::EightBitMime)))
    /// );
    ///
    /// assert_eq!(
    ///     Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY=7BIT"),
    ///     Ok(Event::MailCmd("ned@ymir.claremont.edu".to_string(), Some(EightBitMimeBody::SevenBit)))
    /// );
    ///
    /// assert_eq!(Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> Foo"), Err(SMTPReplyCode::Code504));
    /// assert_eq!(Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY="), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY"), Err(SMTPReplyCode::Code504));
    /// assert_eq!(Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY=bar"), Err(SMTPReplyCode::Code501));
    /// assert_eq!(
    ///     Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY=8BITMIME BODY=7BIT"),
    ///     Err(SMTPReplyCode::Code501)
    /// );
    /// ```
    ///
    /// Mail from Command with Internationalized Email
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, event::EightBitMimeBody, code::SMTPReplyCode};
    ///
    /// assert_eq!(
    ///     Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> SMTPUTF8"),
    ///     Ok(Event::MailCmd("ned@ymir.claremont.edu".to_string(), None))
    /// );
    /// assert_eq!(
    ///     Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> SMTPUTF8=foo"),
    ///     Err(SMTPReplyCode::Code504)
    /// );
    /// assert_eq!(
    ///     Event::parse_cmd("MAIL FROM:<用户@例子.广告> SMTPUTF8"),
    ///     Ok(Event::MailCmd("用户@例子.广告".to_string(), None))
    /// );
    /// ```
    ///
    /// Rcpt to Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// // TODO: RCPT TO:<@hosta.int,@jkl.org:userc@d.bar.org>
    ///
    /// assert_eq!(
    ///     Event::parse_cmd("RcPt To:<valid@forward.path.com>"),
    ///     Ok(Event::RcptCmd("valid@forward.path.com".to_string()))
    /// );
    /// assert_eq!(
    ///     Event::parse_cmd("rCpT TO: <valid2@forward.path.com>"),
    ///     Ok(Event::RcptCmd("valid2@forward.path.com".to_string()))
    /// );
    /// assert_eq!(Event::parse_cmd("RCPT TO:   <>  "), Err(SMTPReplyCode::Code501));
    /// // assert_eq!(
    /// //     Event::parse_cmd("RCPT tO:   <local.part@[127.0.0.1]>  "),
    /// //     Ok(Event::RcptCmd("local.part@[127.0.0.1]".to_string()))
    /// // );
    /// assert_eq!(
    ///     Event::parse_cmd("rcpt to:   <\"john..doe\"@example.org>  "),
    ///     Ok(Event::RcptCmd("\"john..doe\"@example.org".to_string()))
    /// );
    /// assert_eq!(Event::parse_cmd("RCPT TO:   <ibm@com>  extra_arg "), Err(SMTPReplyCode::Code504));
    /// assert_eq!(Event::parse_cmd("RcpT  TO:  "), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("RCPT TO ko"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Rcpt to Command with Internationalized Email
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, event::EightBitMimeBody, code::SMTPReplyCode};
    ///
    /// assert_eq!(
    ///     Event::parse_cmd("RCPT TO:<ned@ymir.claremont.edu> SMTPUTF8"),
    ///     Ok(Event::RcptCmd("ned@ymir.claremont.edu".to_string()))
    /// );
    /// assert_eq!(
    ///     Event::parse_cmd("RCPT TO:<ned@ymir.claremont.edu> SMTPUTF8=foo"),
    ///     Err(SMTPReplyCode::Code504)
    /// );
    /// assert_eq!(
    ///     Event::parse_cmd("RCPT TO:<用户@例子.广告> SMTPUTF8"),
    ///     Ok(Event::RcptCmd("用户@例子.广告".to_string()))
    /// );
    /// ```
    ///
    /// Data Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("DATA"), Ok(Event::DataCmd));
    /// assert_eq!(Event::parse_cmd("dAtA"), Ok(Event::DataCmd));
    /// assert_eq!(Event::parse_cmd("data dummy"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Quit Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("QuIt"), Ok(Event::QuitCmd));
    /// assert_eq!(Event::parse_cmd("quit"), Ok(Event::QuitCmd));
    /// assert_eq!(Event::parse_cmd("QUIT dummy"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Reset Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("rset"), Ok(Event::RsetCmd));
    /// assert_eq!(Event::parse_cmd("RsEt"), Ok(Event::RsetCmd));
    /// assert_eq!(Event::parse_cmd("RSET dummy"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Noop Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("Noop"), Ok(Event::NoopCmd));
    /// assert_eq!(Event::parse_cmd("NOOP"), Ok(Event::NoopCmd));
    /// assert_eq!(Event::parse_cmd("nOoP dummy"), Ok(Event::NoopCmd));
    /// assert_eq!(Event::parse_cmd("noop dummy NOOP"), Ok(Event::NoopCmd));
    /// ```
    ///
    /// Verify Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("VrFy foobar"), Ok(Event::VrfyCmd("foobar".to_string())));
    /// assert_eq!(Event::parse_cmd("VRFY"), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("vrfy     dummy"), Ok(Event::VrfyCmd("dummy".to_string())));
    /// assert_eq!(Event::parse_cmd("vRrY       dummy        toto"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Expand Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("EXPN foobar"), Ok(Event::ExpnCmd("foobar".to_string())));
    /// assert_eq!(Event::parse_cmd("eXpN"), Err(SMTPReplyCode::Code501));
    /// assert_eq!(Event::parse_cmd("eXpN     dummy"), Ok(Event::ExpnCmd("dummy".to_string())));
    /// assert_eq!(Event::parse_cmd("expn       dummy        toto"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Help Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("HELP foobar"), Ok(Event::HelpCmd(Some("foobar".to_string()))));
    /// assert_eq!(Event::parse_cmd("help"), Ok(Event::HelpCmd(None)));
    /// assert_eq!(Event::parse_cmd("hElP     dummy"), Ok(Event::HelpCmd(Some("dummy".to_string()))));
    /// assert_eq!(Event::parse_cmd("hELp       dummy        toto"), Err(SMTPReplyCode::Code501));
    /// ```
    ///
    /// Start tls Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event, code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_cmd("StarTtLs"), Ok(Event::StartTls));
    /// assert_eq!(Event::parse_cmd("STARTTLS"), Ok(Event::StartTls));
    /// assert_eq!(Event::parse_cmd("STARTTLS dummy"), Err(SMTPReplyCode::Code501));
    /// ```
    pub fn parse_cmd(input: &str) -> Result<Event, SMTPReplyCode> {
        // // TODO: if utf8 extension (8BITMIME && SMTPUTF8) are disable
        // // add a smtp.extensions=[...] or smtp.disabled_extensions=[...] in config
        // if !input.is_ascii() {
        //     return Err(SMTPReplyCode::Code500);
        // }

        // 80 - "\r\n".len() (if SMTPUTF8 + 10)
        if input.len() > 88 || input.is_empty() {
            return Err(SMTPReplyCode::Code500);
        }

        let words = input
            .split_whitespace()
            // .inspect(|x| log::trace!(target: RECEIVER, "word:{}", x))
            .collect::<Vec<&str>>();
        if words.is_empty() {
            return Err(SMTPReplyCode::Code500);
        }
        let mut smtp_args = words.iter();
        let smtp_verb = match smtp_args.next() {
            // TODO: verify rfc about that..
            // NOTE: if the first word is not the beginning of the input (whitespace before)
            Some(fist_word) if &input[..fist_word.len()] != *fist_word => {
                return Err(SMTPReplyCode::Code501);
            }
            Some(smtp_verb) => smtp_verb,
            None => return Err(SMTPReplyCode::Code500),
        };
        match (
            smtp_verb.to_ascii_uppercase().as_str(),
            smtp_args.as_slice(),
        ) {
            ("HELO", args) => Event::parse_arg_helo(args),
            ("EHLO", args) => Event::parse_arg_ehlo(args),
            ("MAIL", args) => Event::parse_arg_mail_from(args),
            ("RCPT", args) => Event::parse_arg_rcpt_to(args),

            // TODO: accept no value argument SMTPUTF8
            ("VRFY", [user_or_mailbox]) => Ok(Event::VrfyCmd(user_or_mailbox.to_string())),
            // TODO: accept no value argument SMTPUTF8
            ("EXPN", [mailing_list]) => Ok(Event::ExpnCmd(mailing_list.to_string())),

            ("HELP", []) => Ok(Event::HelpCmd(None)),
            ("HELP", [help_value]) => Ok(Event::HelpCmd(Some(help_value.to_string()))),

            ("DATA", []) => Ok(Event::DataCmd),
            ("QUIT", []) => Ok(Event::QuitCmd),
            ("RSET", []) => Ok(Event::RsetCmd),
            ("NOOP", [..]) => Ok(Event::NoopCmd),

            ("STARTTLS", []) => Ok(Event::StartTls),

            _ => Err(SMTPReplyCode::Code501),
        }
    }

    fn parse_arg_helo(args: &[&str]) -> Result<Event, SMTPReplyCode> {
        match args {
            [domain] => match addr::parse_domain_name(domain) {
                Ok(domain) => Ok(Event::HeloCmd(domain.to_string())),
                Err(_) => Err(SMTPReplyCode::Code501),
            },
            _ => Err(SMTPReplyCode::Code501),
        }
    }

    fn parse_arg_ehlo(args: &[&str]) -> Result<Event, SMTPReplyCode> {
        match args {
            [domain_or_address_literal] => {
                match addr::parse_domain_name(domain_or_address_literal) {
                    Ok(domain) => Ok(Event::EhloCmd(domain.to_string())),
                    // TODO: improve that see https://datatracker.ietf.org/doc/html/rfc5321#section-4.1.3
                    // addr::email::Host::parse
                    Err(_)
                        if domain_or_address_literal.starts_with('[')
                            && domain_or_address_literal.ends_with(']') =>
                    {
                        match domain_or_address_literal[1..domain_or_address_literal.len() - 1]
                            .parse::<std::net::IpAddr>()
                        {
                            Ok(address) => Ok(Event::EhloCmd(address.to_string())),
                            Err(_) => Err(SMTPReplyCode::Code501),
                        }
                    }
                    _ => Err(SMTPReplyCode::Code501),
                }
            }
            _ => Err(SMTPReplyCode::Code501),
        }
    }

    /// Parse a possible reverse or forward path or return nothing
    ///
    /// ```
    /// use vsmtp::smtp::event::Event;
    ///
    /// assert_eq!(Event::from_path("foo@bar", false), None);
    /// assert_eq!(Event::from_path("foo@bar", true), None);
    /// assert_eq!(Event::from_path("<>", true), Some("".to_string()));
    /// assert_eq!(Event::from_path("<>", false), None);
    ///
    /// assert_eq!(Event::from_path("<foo@bar>", false), Some("foo@bar".to_string()));
    /// assert_eq!(Event::from_path("<not-a-valid-address>", false), None);
    ///
    /// assert_eq!(Event::from_path("<simple@examplecom>", false), Some("simple@examplecom".to_string()));
    /// assert_eq!(Event::from_path("<simple@example.com>", false), Some("simple@example.com".to_string()));
    /// assert_eq!(Event::from_path("<very.common@example.com>", false), Some("very.common@example.com".to_string()));
    /// assert_eq!(
    ///     Event::from_path("<disposable.style.email.with+symbol@example.com>", false),
    ///     Some("disposable.style.email.with+symbol@example.com".to_string())
    /// );
    /// assert_eq!(
    ///     Event::from_path("<other.email-with-hyphen@example.com>", false),
    ///     Some("other.email-with-hyphen@example.com".to_string())
    /// );
    /// assert_eq!(
    ///     Event::from_path("<fully-qualified-domain@example.com>", false),
    ///     Some("fully-qualified-domain@example.com".to_string())
    /// );
    /// assert_eq!(
    ///     Event::from_path("<user.name+tag+sorting@example.com>", false),
    ///     Some("user.name+tag+sorting@example.com".to_string())
    /// );
    /// assert_eq!(Event::from_path("<x@example.com>", false), Some("x@example.com".to_string()));
    /// assert_eq!(
    ///     Event::from_path("<example-indeed@strange-example.com>", false),
    ///     Some("example-indeed@strange-example.com".to_string())
    /// );
    /// assert_eq!(Event::from_path("<test/test@test.com>", false), Some("test/test@test.com".to_string()));
    /// assert_eq!(Event::from_path("<admin@mailserver1>", false), Some("admin@mailserver1".to_string()));
    /// assert_eq!(Event::from_path("<example@s.example>", false), Some("example@s.example".to_string()));
    /// assert_eq!(Event::from_path("<\" \"@example.org>", false), Some("\" \"@example.org".to_string()));
    /// assert_eq!(
    ///     Event::from_path("<\"john..doe\"@example.org>", false),
    ///     Some("\"john..doe\"@example.org".to_string())
    /// );
    /// assert_eq!(
    ///     Event::from_path("<mailhost!username@example.org>", false),
    ///     Some("mailhost!username@example.org".to_string())
    /// );
    /// assert_eq!(
    ///     Event::from_path("<user%example.com@example.org>", false),
    ///     Some("user%example.com@example.org".to_string())
    /// );
    /// assert_eq!(Event::from_path("<user-@example.org>", false), Some("user-@example.org".to_string()));
    ///
    /// assert_eq!(Event::from_path("<用户@例子.广告>", false), Some("用户@例子.广告".to_string()));
    /// assert_eq!(Event::from_path("<अजय@डाटा.भारत>", false), Some("अजय@डाटा.भारत".to_string()));
    /// assert_eq!(Event::from_path("<квіточка@пошта.укр>", false), Some("квіточка@пошта.укр".to_string()));
    /// assert_eq!(Event::from_path("<χρήστης@παράδειγμα.ελ>", false), Some("χρήστης@παράδειγμα.ελ".to_string()));
    /// assert_eq!(
    ///     Event::from_path("<Dörte@Sörensen.example.com>", false),
    ///     Some("Dörte@Sörensen.example.com".to_string())
    /// );
    /// assert_eq!(Event::from_path("<коля@пример.рф>", false), Some("коля@пример.рф".to_string()));
    /// ```
    pub fn from_path(input: &str, may_be_empty: bool) -> Option<String> {
        if input.starts_with('<') && input.ends_with('>') {
            match &input[1..input.len() - 1] {
                "" if may_be_empty => Some("".to_string()),
                // TODO: should accept and ignore A-d-l ("source route")
                // https://datatracker.ietf.org/doc/html/rfc5321#section-4.1.2
                mailbox => match addr::parse_email_address(mailbox) {
                    Ok(mailbox) => Some(mailbox.to_string()),
                    Err(_) => None,
                },
            }
        } else {
            None
        }
    }

    fn parse_arg_mail_from(args: &[&str]) -> Result<Event, SMTPReplyCode> {
        fn parse_esmtp_args(path: String, args: &[&str]) -> Result<Event, SMTPReplyCode> {
            let mut bitmime = None;

            for arg in args {
                if let Some(raw) = arg.strip_prefix("BODY=") {
                    if bitmime.is_none() {
                        bitmime = Some(<EightBitMimeBody as std::str::FromStr>::from_str(raw)?);
                    } else {
                        return Err(SMTPReplyCode::Code501);
                    }
                } else if *arg == "SMTPUTF8" {
                    // TODO: ?
                } else {
                    return Err(SMTPReplyCode::Code504);
                }
            }

            Ok(Event::MailCmd(path, bitmime))
        }

        match args {
            // note: separated word (can return a warning)
            [from, reverse_path, ..] if from.to_ascii_uppercase() == "FROM:" => {
                match Event::from_path(reverse_path, true) {
                    Some(path) => parse_esmtp_args(path, &args[2..]),
                    None => Err(SMTPReplyCode::Code501),
                }
            }
            [from_and_reverse_path, ..] => match from_and_reverse_path
                .to_ascii_uppercase()
                .strip_prefix("FROM:")
            {
                Some("") | None => Err(SMTPReplyCode::Code501),
                Some(_) => match Event::from_path(&from_and_reverse_path["FROM:".len()..], true) {
                    Some(path) => parse_esmtp_args(path, &args[1..]),
                    None => Err(SMTPReplyCode::Code501),
                },
            },
            _ => Err(SMTPReplyCode::Code501),
        }
    }

    fn parse_arg_rcpt_to(args: &[&str]) -> Result<Event, SMTPReplyCode> {
        // TODO: https://datatracker.ietf.org/doc/html/rfc5321#section-4.1.1.3
        // If service extensions were negotiated, the RCPT command may also
        // carry parameters associated with a particular service extension
        // offered by the server.  The client MUST NOT transmit parameters other
        // than those associated with a service extension offered by the server
        // in its EHLO response.
        //
        // Syntax:
        //
        //     rcpt = "RCPT TO:" ( "<Postmaster@" Domain ">" / "<Postmaster>" /
        //         Forward-path ) [SP Rcpt-parameters] CRLF
        //
        // Note that, in a departure from the usual rules for
        // local-parts, the "Postmaster" string shown above is
        // treated as case-insensitive.

        fn parse_esmtp_args(path: String, args: &[&str]) -> Result<Event, SMTPReplyCode> {
            for arg in args {
                if *arg == "SMTPUTF8" {
                    // TODO: ?
                } else {
                    return Err(SMTPReplyCode::Code504);
                }
            }

            Ok(Event::RcptCmd(path))
        }

        match args {
            // NOTE: separated word (can return a warning)
            [to, forward_path, ..] if to.to_ascii_uppercase() == "TO:" => {
                match Event::from_path(forward_path, false) {
                    Some(path) => parse_esmtp_args(path, &args[2..]),
                    None => Err(SMTPReplyCode::Code501),
                }
            }
            [to_and_forward_path, ..] => {
                match to_and_forward_path.to_ascii_uppercase().strip_prefix("TO:") {
                    Some("") | None => Err(SMTPReplyCode::Code501),
                    Some(_) => match Event::from_path(&to_and_forward_path["TO:".len()..], false) {
                        Some(path) => parse_esmtp_args(path, &args[1..]),
                        None => Err(SMTPReplyCode::Code501),
                    },
                }
            }
            _ => Err(SMTPReplyCode::Code501),
        }
    }

    /// Data Command
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event,code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_data("."), Ok(Event::DataEnd));
    /// ```
    ///
    /// ```
    /// use vsmtp::smtp::{event::Event,code::SMTPReplyCode};
    ///
    /// assert_eq!(Event::parse_data(""), Ok(Event::DataLine("".to_string())));
    /// assert_eq!(Event::parse_data("foobar helo"), Ok(Event::DataLine("foobar helo".to_string())));
    /// assert_eq!(Event::parse_data("இந்தியா"), Ok(Event::DataLine("இந்தியா".to_string())));
    /// assert_eq!(Event::parse_data("网络"), Ok(Event::DataLine("网络".to_string())));
    /// assert_eq!(Event::parse_data("भारत"), Ok(Event::DataLine("भारत".to_string())));
    /// assert_eq!(
    ///     Event::parse_data("가각간갈감갑갇강개객거건걸검겁겨게격견결겸겹경계"),
    ///     Ok(Event::DataLine("가각간갈감갑갇강개객거건걸검겁겨게격견결겸겹경계".to_string()))
    /// );
    ///
    /// assert_eq!(
    ///     Event::parse_data(std::str::from_utf8(&vec![b'_'; 1000][..]).unwrap()),
    ///     Err(SMTPReplyCode::Code500)
    /// );
    /// ```
    pub fn parse_data(input: &str) -> Result<Event, SMTPReplyCode> {
        match input {
            "." => Ok(Event::DataEnd),
            too_long if too_long.len() > 998 => Err(SMTPReplyCode::Code500),
            _ => Ok(Event::DataLine(input.to_string())),
        }
    }
}
