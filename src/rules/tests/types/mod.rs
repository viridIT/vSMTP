use crate::rules::{
    address::Address,
    rule_engine::{RuleEngine, Status},
    tests::helpers::get_default_state,
};

#[test]
fn test_status() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/types/status").expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_time() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/types/time").expect("couldn't build rule engine");
    let mut state = get_default_state();

    state.add_data("time", std::time::SystemTime::UNIX_EPOCH);

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_socket() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re = RuleEngine::new("./src/rules/tests/types/socket").expect("couldn't build rule engine");
    let mut state = get_default_state();

    state.add_data(
        "custom_socket",
        <std::net::SocketAddr as std::str::FromStr>::from_str("127.0.0.1:25")
            .expect("could not build socket"),
    );

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_address() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re =
        RuleEngine::new("./src/rules/tests/types/address").expect("couldn't build rule engine");
    let mut state = get_default_state();

    state.get_context().write().unwrap().envelop.mail_from =
        Address::new("mail.from@test.net").expect("could not parse address");

    assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
}

#[test]
fn test_objects() {
    crate::receiver::test_helpers::logs::setup_logs();

    let re =
        RuleEngine::new("./src/rules/tests/types/objects").expect("couldn't build rule engine");
    let mut state = get_default_state();

    assert_eq!(re.run_when(&mut state, "connect"), Status::Next);
}
