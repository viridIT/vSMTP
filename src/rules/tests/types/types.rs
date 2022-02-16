#[cfg(test)]
pub mod test_types {
    use crate::rules::{
        rule_engine::{RuleEngine, RuleState, Status},
        tests::helpers::get_default_state,
    };
    use std::io::Read;

    #[test]
    fn test_status() {
        crate::receiver::test_helpers::logs::setup_logs();

        let re = RuleEngine::new("./src/rules/tests/rules/types/status")
            .expect("couldn't build rule engine");
        let mut state = get_default_state();

        assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);

        let mut buffer = String::default();
        let stdout = std::io::stdin()
            .read_to_string(&mut buffer)
            .expect("no print emitted");

        assert_eq!(buffer.as_str(), "next\nnext");
    }
}
