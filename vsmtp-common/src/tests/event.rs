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
use crate::{
    code::SMTPReplyCode,
    event::{Event, MimeBodyType},
};

#[test]
fn data_end() {
    assert_eq!(Event::parse_data("."), Ok(Event::DataEnd));
}

#[test]
fn dot_stuffing() {
    assert_eq!(
        Event::parse_data(".."),
        Ok(Event::DataLine(".".to_string()))
    );
    assert_eq!(
        Event::parse_data(".How are you today?"),
        Ok(Event::DataLine("How are you today?".to_string()))
    );
}

#[test]
fn data_valid() {
    assert_eq!(Event::parse_data(""), Ok(Event::DataLine("".to_string())));
    assert_eq!(
        Event::parse_data("foobar helo"),
        Ok(Event::DataLine("foobar helo".to_string()))
    );
    assert_eq!(
        Event::parse_data("இந்தியா"),
        Ok(Event::DataLine("இந்தியா".to_string()))
    );
    assert_eq!(
        Event::parse_data("网络"),
        Ok(Event::DataLine("网络".to_string()))
    );
    assert_eq!(
        Event::parse_data("भारत"),
        Ok(Event::DataLine("भारत".to_string()))
    );
    assert_eq!(
        Event::parse_data("가각간갈감갑갇강개객거건걸검겁겨게격견결겸겹경계"),
        Ok(Event::DataLine(
            "가각간갈감갑갇강개객거건걸검겁겨게격견결겸겹경계".to_string()
        ))
    );
}

#[test]
fn data_invalid() {
    assert_eq!(
        Event::parse_data(std::str::from_utf8(&vec![b'_'; 1000][..]).unwrap()),
        Err(SMTPReplyCode::Code500)
    );
}

#[test]
fn command_invalid() {
    assert_eq!(Event::parse_cmd(""), Err(SMTPReplyCode::Code500));
    assert_eq!(Event::parse_cmd("     "), Err(SMTPReplyCode::Code500));
    assert_eq!(
        Event::parse_cmd(std::str::from_utf8(&vec![b'_'; 100][..]).unwrap()),
        Err(SMTPReplyCode::Code500)
    );
    assert_eq!(Event::parse_cmd("    foo"), Err(SMTPReplyCode::Code501));
}

#[test]
fn command_helo() {
    assert_eq!(
        Event::parse_cmd("HELO foobar"),
        Ok(Event::HeloCmd("foobar".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("hElO   ibm.com  "),
        Ok(Event::HeloCmd("ibm.com".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("HELO [127.0.0.1]"),
        Ok(Event::HeloCmd("127.0.0.1".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("hElO  not\\a.valid\"domain"),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(Event::parse_cmd("hElO  "), Err(SMTPReplyCode::Code501));
    assert_eq!(
        Event::parse_cmd("   hElO  valid_domain"),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::parse_cmd("HELO one two"),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::parse_cmd("HELO 0.0.0.0"),
        Err(SMTPReplyCode::Code501)
    );
}

#[test]
fn ehlo_command() {
    assert_eq!(
        Event::parse_cmd("EHLO foobar"),
        Ok(Event::EhloCmd("foobar".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("EHLO   ibm.com  "),
        Ok(Event::EhloCmd("ibm.com".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("hElO  not\\a.valid\"domain"),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::parse_cmd("EHLO   [127.0.0.1]  "),
        Ok(Event::EhloCmd("127.0.0.1".to_string()))
    );

    assert!(Event::parse_cmd("EHLO   [foobar]  ").is_err(),);
    assert_eq!(
        Event::parse_cmd("ehlo   [0011:2233:4455:6677:8899:aabb:ccdd:eeff]  "),
        Ok(Event::EhloCmd(
            "11:2233:4455:6677:8899:aabb:ccdd:eeff".to_string()
        ))
    );
    assert_eq!(Event::parse_cmd("EHLO  "), Err(SMTPReplyCode::Code501));
    assert_eq!(
        Event::parse_cmd("   EHLO  valid_domain"),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::parse_cmd("EHLO one two"),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::parse_cmd("EHLO 0.0.0.0"),
        Err(SMTPReplyCode::Code501)
    );
}

#[test]
fn command_mail_from() {
    assert_eq!(
        Event::parse_cmd("Mail FROM:<valid@reverse.path.com>"),
        Ok(Event::MailCmd("valid@reverse.path.com".to_string(), None))
    );
    assert_eq!(
        Event::parse_cmd("Mail fRoM: <valid2@reverse.path.com>"),
        Ok(Event::MailCmd("valid2@reverse.path.com".to_string(), None))
    );
    assert_eq!(
        Event::parse_cmd("MaIl From:   <>  "),
        Ok(Event::MailCmd("".to_string(), None))
    );
    // assert_eq!(
    //     Event::parse_cmd("MaIl From:   <local.part@[127.0.0.1]>  "),
    //     Ok(Event::MailCmd("local.part@[127.0.0.1]".to_string(), None))
    // );
    assert_eq!(
        Event::parse_cmd("MaIl From:   <\"john..doe\"@example.org>  "),
        Ok(Event::MailCmd(
            "\"john..doe\"@example.org".to_string(),
            None
        ))
    );
    assert_eq!(
        Event::parse_cmd("Mail  From:  "),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::parse_cmd("Mail From ko"),
        Err(SMTPReplyCode::Code501)
    );

    assert_eq!(Event::parse_cmd("Mail"), Err(SMTPReplyCode::Code501));
}

#[test]
fn command_mail_from_8bitmime() {
    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY=8BITMIME"),
        Ok(Event::MailCmd(
            "ned@ymir.claremont.edu".to_string(),
            Some(MimeBodyType::EightBitMime)
        ))
    );

    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY=7BIT"),
        Ok(Event::MailCmd(
            "ned@ymir.claremont.edu".to_string(),
            Some(MimeBodyType::SevenBit)
        ))
    );

    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> Foo"),
        Err(SMTPReplyCode::Code504)
    );
    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY="),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY"),
        Err(SMTPReplyCode::Code504)
    );
    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY=bar"),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> BODY=8BITMIME BODY=7BIT"),
        Err(SMTPReplyCode::Code501)
    );
}

#[test]
fn command_mail_from_international() {
    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> SMTPUTF8"),
        Ok(Event::MailCmd("ned@ymir.claremont.edu".to_string(), None))
    );
    assert_eq!(
        Event::parse_cmd("MAIL FROM:<ned@ymir.claremont.edu> SMTPUTF8=foo"),
        Err(SMTPReplyCode::Code504)
    );
    assert_eq!(
        Event::parse_cmd("MAIL FROM:<用户@例子.广告> SMTPUTF8"),
        Ok(Event::MailCmd("用户@例子.广告".to_string(), None))
    );
}

#[test]
fn command_rcpt_to() {
    // TODO: RCPT TO:<@hosta.int,@jkl.org:userc@d.bar.org>

    assert_eq!(
        Event::parse_cmd("RcPt To:<valid@forward.path.com>"),
        Ok(Event::RcptCmd("valid@forward.path.com".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("rCpT TO: <valid2@forward.path.com>"),
        Ok(Event::RcptCmd("valid2@forward.path.com".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("RCPT TO:   <>  "),
        Err(SMTPReplyCode::Code501)
    );
    // assert_eq!(
    //     Event::parse_cmd("RCPT tO:   <local.part@[127.0.0.1]>  "),
    //     Ok(Event::RcptCmd("local.part@[127.0.0.1]".to_string()))
    // );
    assert_eq!(
        Event::parse_cmd("rcpt to:   <\"john..doe\"@example.org>  "),
        Ok(Event::RcptCmd("\"john..doe\"@example.org".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("RCPT TO:   <ibm@com>  extra_arg "),
        Err(SMTPReplyCode::Code504)
    );
    assert_eq!(Event::parse_cmd("RcpT  TO:  "), Err(SMTPReplyCode::Code501));
    assert_eq!(Event::parse_cmd("RCPT TO ko"), Err(SMTPReplyCode::Code501));

    assert_eq!(Event::parse_cmd("RCPT"), Err(SMTPReplyCode::Code501));
}

#[test]
fn command_rcpt_to_international() {
    assert_eq!(
        Event::parse_cmd("RCPT TO:<用户@例子.广告>"),
        Ok(Event::RcptCmd("用户@例子.广告".to_string()))
    );
}

#[test]
fn command_data() {
    assert_eq!(Event::parse_cmd("DATA"), Ok(Event::DataCmd));
    assert_eq!(Event::parse_cmd("dAtA"), Ok(Event::DataCmd));
    assert_eq!(Event::parse_cmd("data dummy"), Err(SMTPReplyCode::Code501));
}

#[test]
fn command_quit() {
    assert_eq!(Event::parse_cmd("QuIt"), Ok(Event::QuitCmd));
    assert_eq!(Event::parse_cmd("quit"), Ok(Event::QuitCmd));
    assert_eq!(Event::parse_cmd("QUIT dummy"), Err(SMTPReplyCode::Code501));
}

#[test]
fn command_rset() {
    assert_eq!(Event::parse_cmd("rset"), Ok(Event::RsetCmd));
    assert_eq!(Event::parse_cmd("RsEt"), Ok(Event::RsetCmd));
    assert_eq!(Event::parse_cmd("RSET dummy"), Err(SMTPReplyCode::Code501));
}

#[test]
fn command_noop() {
    assert_eq!(Event::parse_cmd("Noop"), Ok(Event::NoopCmd));
    assert_eq!(Event::parse_cmd("NOOP"), Ok(Event::NoopCmd));
    assert_eq!(Event::parse_cmd("nOoP dummy"), Ok(Event::NoopCmd));
    assert_eq!(Event::parse_cmd("noop dummy NOOP"), Ok(Event::NoopCmd));
}

#[test]
fn command_verify() {
    assert_eq!(
        Event::parse_cmd("VrFy foobar"),
        Ok(Event::VrfyCmd("foobar".to_string()))
    );
    assert_eq!(Event::parse_cmd("VRFY"), Err(SMTPReplyCode::Code501));
    assert_eq!(
        Event::parse_cmd("vrfy     dummy"),
        Ok(Event::VrfyCmd("dummy".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("vrfy     dummy SMTPUTF8"),
        Ok(Event::VrfyCmd("dummy".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("vRrY       dummy        toto"),
        Err(SMTPReplyCode::Code501)
    );
}

#[test]
fn command_expand() {
    assert_eq!(
        Event::parse_cmd("EXPN foobar"),
        Ok(Event::ExpnCmd("foobar".to_string()))
    );
    assert_eq!(Event::parse_cmd("eXpN"), Err(SMTPReplyCode::Code501));
    assert_eq!(
        Event::parse_cmd("eXpN     dummy"),
        Ok(Event::ExpnCmd("dummy".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("eXpN     dummy SMTPUTF8"),
        Ok(Event::ExpnCmd("dummy".to_string()))
    );
    assert_eq!(
        Event::parse_cmd("expn       dummy        toto"),
        Err(SMTPReplyCode::Code501)
    );
}

#[test]
fn command_help() {
    assert_eq!(
        Event::parse_cmd("HELP foobar"),
        Ok(Event::HelpCmd(Some("foobar".to_string())))
    );
    assert_eq!(Event::parse_cmd("help"), Ok(Event::HelpCmd(None)));
    assert_eq!(
        Event::parse_cmd("hElP     dummy"),
        Ok(Event::HelpCmd(Some("dummy".to_string())))
    );
    assert_eq!(
        Event::parse_cmd("hELp       dummy        toto"),
        Err(SMTPReplyCode::Code501)
    );
}

#[test]
fn command_starttls() {
    assert_eq!(Event::parse_cmd("StarTtLs"), Ok(Event::StartTls));
    assert_eq!(Event::parse_cmd("STARTTLS"), Ok(Event::StartTls));
    assert_eq!(
        Event::parse_cmd("STARTTLS dummy"),
        Err(SMTPReplyCode::Code501)
    );
}

#[test]
#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn parse_path() {
    assert_eq!(
        Event::from_path("foo@bar", false),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(
        Event::from_path("foo@bar", true),
        Err(SMTPReplyCode::Code501)
    );
    assert_eq!(Event::from_path("<>", true), Ok("".to_string()));
    assert_eq!(Event::from_path("<>", false), Err(SMTPReplyCode::Code501));

    assert_eq!(
        Event::from_path("<foo@bar>", false),
        Ok("foo@bar".to_string())
    );
    assert_eq!(
        Event::from_path("<not-a-valid-address>", false),
        Err(SMTPReplyCode::Code501)
    );

    assert_eq!(
        Event::from_path("<simple@examplecom>", false),
        Ok("simple@examplecom".to_string())
    );
    assert_eq!(
        Event::from_path("<simple@example.com>", false),
        Ok("simple@example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<very.common@example.com>", false),
        Ok("very.common@example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<disposable.style.email.with+symbol@example.com>", false),
        Ok("disposable.style.email.with+symbol@example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<other.email-with-hyphen@example.com>", false),
        Ok("other.email-with-hyphen@example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<fully-qualified-domain@example.com>", false),
        Ok("fully-qualified-domain@example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<user.name+tag+sorting@example.com>", false),
        Ok("user.name+tag+sorting@example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<x@example.com>", false),
        Ok("x@example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<example-indeed@strange-example.com>", false),
        Ok("example-indeed@strange-example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<test/test@test.com>", false),
        Ok("test/test@test.com".to_string())
    );
    assert_eq!(
        Event::from_path("<admin@mailserver1>", false),
        Ok("admin@mailserver1".to_string())
    );
    assert_eq!(
        Event::from_path("<example@s.example>", false),
        Ok("example@s.example".to_string())
    );
    assert_eq!(
        Event::from_path("<\" \"@example.org>", false),
        Ok("\" \"@example.org".to_string())
    );
    assert_eq!(
        Event::from_path("<\"john..doe\"@example.org>", false),
        Ok("\"john..doe\"@example.org".to_string())
    );
    assert_eq!(
        Event::from_path("<mailhost!username@example.org>", false),
        Ok("mailhost!username@example.org".to_string())
    );
    assert_eq!(
        Event::from_path("<user%example.com@example.org>", false),
        Ok("user%example.com@example.org".to_string())
    );
    assert_eq!(
        Event::from_path("<user-@example.org>", false),
        Ok("user-@example.org".to_string())
    );

    assert_eq!(
        Event::from_path("<用户@例子.广告>", false),
        Ok("用户@例子.广告".to_string())
    );
    assert_eq!(
        Event::from_path("<अजय@डाटा.भारत>", false),
        Ok("अजय@डाटा.भारत".to_string())
    );
    assert_eq!(
        Event::from_path("<квіточка@пошта.укр>", false),
        Ok("квіточка@пошта.укр".to_string())
    );
    assert_eq!(
        Event::from_path("<χρήστης@παράδειγμα.ελ>", false),
        Ok("χρήστης@παράδειγμα.ελ".to_string())
    );
    assert_eq!(
        Event::from_path("<Dörte@Sörensen.example.com>", false),
        Ok("Dörte@Sörensen.example.com".to_string())
    );
    assert_eq!(
        Event::from_path("<коля@пример.рф>", false),
        Ok("коля@пример.рф".to_string())
    );
}
