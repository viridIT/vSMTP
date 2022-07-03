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
use crate::rule_state::RuleState;
use crate::tests::helpers::get_default_config;
use crate::{rule_engine::RuleEngine, tests::helpers::get_default_state};
use vsmtp_common::auth::Mechanism;
use vsmtp_common::re::serde_json;
use vsmtp_common::transfer::ForwardTarget;
use vsmtp_common::{
    mail_context::MessageMetadata, state::StateSMTP, status::Status, transfer::Transfer,
    MessageBody,
};
use vsmtp_common::{CodeID, ReplyOrCodeID};
use vsmtp_config::field::FieldServerVirtual;
use vsmtp_mail_parser::MailMimeParser;

#[test]
fn test_logs() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/logs.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Deny(ReplyOrCodeID::Left(CodeID::Denied))
    );
}

#[test]
fn test_users() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/utils.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Delivery),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );
}

#[test]
fn test_send_mail() {
    let (mut state, config) = get_default_state(format!("{}", root_example!["actions"].display()));
    let re = RuleEngine::new(&config, &Some(root_example!["actions/send_mail.vsl"])).unwrap();

    // TODO: add test to send a valid email.
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );
}

#[test]
fn test_context_write() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/write.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    state.context().write().unwrap().metadata = Some(MessageMetadata {
        message_id: "test_message_id".to_string(),
        timestamp: std::time::SystemTime::now(),
        skipped: None,
    });
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::MailFrom),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );
    *state.message().write().unwrap() = MessageBody::try_from(concat!(
        "From: john doe <john@doe.com>\r\n",
        "To: green@foo.net\r\n",
        "Subject: test email\r\n",
        "\r\n",
        "This is a raw email.\r\n",
    ))
    .unwrap();
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PreQ),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PostQ),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );

    // raw mail should have been written on disk.
    assert_eq!(
        std::fs::read_to_string("./tmp/app/tests/generated/test_message_id.eml")
            .expect("could not read 'test_message_id'"),
        [
            "From: john doe <john@doe.com>\r\n",
            "To: green@foo.net\r\n",
            "Subject: test email\r\n",
            "\r\n",
            "This is a raw email.\r\n"
        ]
        .concat()
    );

    std::fs::remove_file("./tmp/app/tests/generated/test_message_id.eml")
        .expect("could not remove generated test file");
}

#[test]
fn test_context_dump() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/dump.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    state.context().write().unwrap().metadata = Some(MessageMetadata {
        message_id: "test_message_id".to_string(),
        timestamp: std::time::SystemTime::now(),
        skipped: None,
    });
    *state.message().write().unwrap() = MessageBody::default();
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PreQ),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );

    *state.message().write().unwrap() = MessageBody::try_from(concat!(
        "From: john@doe.com\r\n",
        "To: green@bar.net\r\n",
        "X-Custom-Header: my header\r\n",
        "Date: toto\r\n",
        "\r\n",
        "this is an empty body\r\n",
    ))
    .unwrap();
    state
        .message()
        .write()
        .unwrap()
        .parse::<MailMimeParser>()
        .unwrap();

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PostQ),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );

    assert_eq!(
        std::fs::read_to_string("./tmp/app/tests/generated/test_message_id.json")
            .expect("could not read 'test_message_id'"),
        serde_json::to_string_pretty(&*state.context().read().unwrap())
            .expect("couldn't convert context into string")
    );

    std::fs::remove_file("./tmp/app/tests/generated/test_message_id.json")
        .expect("could not remove generated test file");
}

#[test]
fn test_quarantine() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/quarantine.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    state.context().write().unwrap().metadata = Some(MessageMetadata {
        message_id: "test_message_id".to_string(),
        timestamp: std::time::SystemTime::now(),
        skipped: None,
    });
    *state.message().write().unwrap() = MessageBody::default();
    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PreQ),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );

    assert!(state
        .context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .all(|rcpt| rcpt.transfer_method == Transfer::None));

    *state.message().write().unwrap() = MessageBody::try_from(concat!(
        "From: john@doe.com\r\n",
        "To: green@bar.net\r\n",
        "Date: toto\r\n",
        "X-Custom-Header: my header\r\n",
        "\r\n",
        "this is an empty body\r\n",
    ))
    .unwrap();
    state
        .message()
        .write()
        .unwrap()
        .parse::<MailMimeParser>()
        .unwrap();

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PostQ),
        Status::Quarantine("tests/generated/quarantine2".to_string())
    );
}

#[test]
fn test_forward() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/forward.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");
    *state.message().write().unwrap() = MessageBody::try_from(concat!(
        "From: john@doe.com\r\n",
        "To: green@bar.net\r\n",
        "Date: toto\r\n",
        "X-Custom-Header: my header\r\n",
        "\r\n",
        "this is an empty body\r\n",
    ))
    .unwrap();
    state
        .message()
        .write()
        .unwrap()
        .parse::<MailMimeParser>()
        .unwrap();

    assert_eq!(re.run_when(&mut state, &StateSMTP::Connect), Status::Next);
    assert_eq!(re.run_when(&mut state, &StateSMTP::Delivery), Status::Next);

    let rcpt = state.context().read().unwrap().envelop.rcpt.clone();

    assert_eq!(rcpt[0].address.full(), "fqdn@example.com");
    assert_eq!(
        rcpt[0].transfer_method,
        Transfer::Forward(ForwardTarget::Domain("localhost".to_string()))
    );
    assert_eq!(rcpt[1].address.full(), "ip4@example.com");
    assert_eq!(
        rcpt[1].transfer_method,
        Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V4(
            <std::net::Ipv4Addr as std::str::FromStr>::from_str("127.0.0.1").unwrap()
        )))
    );
    assert_eq!(rcpt[2].address.full(), "ip6@example.com");
    assert_eq!(
        rcpt[2].transfer_method,
        Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V6(
            <std::net::Ipv6Addr as std::str::FromStr>::from_str("::1").unwrap()
        )))
    );
    assert_eq!(rcpt[3].address.full(), "object.str@example.com");
    assert_eq!(
        rcpt[3].transfer_method,
        Transfer::Forward(ForwardTarget::Domain("localhost".to_string()))
    );
    assert_eq!(rcpt[4].address.full(), "object.ip4@example.com");
    assert_eq!(
        rcpt[4].transfer_method,
        Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V4(
            <std::net::Ipv4Addr as std::str::FromStr>::from_str("127.0.0.1").unwrap()
        )))
    );
    assert_eq!(rcpt[5].address.full(), "object.ip6@example.com");
    assert_eq!(
        rcpt[5].transfer_method,
        Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V6(
            <std::net::Ipv6Addr as std::str::FromStr>::from_str("::1").unwrap()
        )))
    );
    assert_eq!(rcpt[6].address.full(), "object.fqdn@example.com");
    assert_eq!(
        rcpt[6].transfer_method,
        Transfer::Forward(ForwardTarget::Domain("test.eu".to_string()))
    );
    assert_eq!(rcpt[7].address.full(), "socket4@example.com");
    assert_eq!(
        rcpt[7].transfer_method,
        Transfer::Forward(ForwardTarget::Socket("127.0.0.1:25".parse().unwrap()))
    );
    assert_eq!(rcpt[8].address.full(), "socket6@example.com");
    assert_eq!(
        rcpt[8].transfer_method,
        Transfer::Forward(ForwardTarget::Socket("[::1]:25".parse().unwrap()))
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn test_forward_all() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/forward_all.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");
    *state.message().write().unwrap() = MessageBody::try_from(concat!(
        "From: john@doe.com\r\n",
        "To: green@bar.net\r\n",
        "Date: toto\r\n",
        "X-Custom-Header: my header\r\n",
        "\r\n",
        "this is an empty body\r\n",
    ))
    .unwrap();
    state
        .message()
        .write()
        .unwrap()
        .parse::<MailMimeParser>()
        .unwrap();

    re.run_when(&mut state, &StateSMTP::Connect);

    re.run_when(&mut state, &StateSMTP::Authenticate(Mechanism::Login, None));

    state
        .context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .for_each(|rcpt| {
            assert_eq!(
                rcpt.transfer_method,
                Transfer::Forward(ForwardTarget::Domain("localhost".to_string()))
            );
        });

    re.run_when(&mut state, &StateSMTP::MailFrom);

    state
        .context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .for_each(|rcpt| {
            assert_eq!(
                rcpt.transfer_method,
                Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V4(
                    <std::net::Ipv4Addr as std::str::FromStr>::from_str("127.0.0.1").unwrap()
                )))
            );
        });

    re.run_when(&mut state, &StateSMTP::RcptTo);

    state
        .context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .for_each(|rcpt| {
            assert_eq!(
                rcpt.transfer_method,
                Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V6(
                    <std::net::Ipv6Addr as std::str::FromStr>::from_str("::1").unwrap()
                )))
            );
        });

    re.run_when(&mut state, &StateSMTP::Data);

    state
        .context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .for_each(|rcpt| {
            assert_eq!(
                rcpt.transfer_method,
                Transfer::Forward(ForwardTarget::Domain("localhost".to_string()))
            );
        });

    re.run_when(&mut state, &StateSMTP::PreQ);

    state
        .context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .for_each(|rcpt| {
            assert_eq!(
                rcpt.transfer_method,
                Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V4(
                    <std::net::Ipv4Addr as std::str::FromStr>::from_str("127.0.0.1").unwrap()
                )))
            );
        });

    re.run_when(&mut state, &StateSMTP::PostQ);

    state
        .context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .for_each(|rcpt| {
            assert_eq!(
                rcpt.transfer_method,
                Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V6(
                    <std::net::Ipv6Addr as std::str::FromStr>::from_str("::1").unwrap()
                )))
            );
        });

    re.run_when(&mut state, &StateSMTP::Delivery);

    state
        .context()
        .read()
        .unwrap()
        .envelop
        .rcpt
        .iter()
        .for_each(|rcpt| {
            assert_eq!(
                rcpt.transfer_method,
                Transfer::Forward(ForwardTarget::Domain("test.eu".to_string()))
            );
        });
}

#[test]
fn test_hostname() {
    let re = RuleEngine::new(
        &vsmtp_config::Config::default(),
        &Some(root_example!["actions/utils.vsl"]),
    )
    .unwrap();
    let (mut state, _) = get_default_state("./tmp/app");

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PostQ),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );
}

#[test]
fn test_in_domain_and_server_name() {
    let (mut state, config) = get_default_state("./tmp/app");
    let re = RuleEngine::new(&config, &Some(root_example!["actions/utils.vsl"])).unwrap();

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::Connect),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );
}

#[test]
fn test_in_domain_and_server_name_sni() {
    let mut config = get_default_config("./tmp/app");
    config.server.r#virtual = std::collections::BTreeMap::from_iter([
        ("example.com".to_string(), FieldServerVirtual::new()),
        ("doe.com".to_string(), FieldServerVirtual::new()),
        ("green.com".to_string(), FieldServerVirtual::new()),
    ]);

    let re = RuleEngine::new(&config, &Some(root_example!["actions/utils.vsl"])).unwrap();
    let resolvers = std::sync::Arc::new(std::collections::HashMap::new());
    let mut state = RuleState::new(&config, resolvers, &re);

    assert_eq!(
        re.run_when(&mut state, &StateSMTP::PreQ),
        Status::Accept(ReplyOrCodeID::Left(CodeID::Ok)),
    );
}
