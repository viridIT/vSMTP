#[cfg(test)]
pub mod test_types {
    use crate::rules::{
        rule_engine::{RuleEngine, Status},
        tests::helpers::get_default_state,
    };

    #[test]
    fn test_status() {
        crate::receiver::test_helpers::logs::setup_logs();

        let re =
            RuleEngine::new("./src/rules/tests/types/status").expect("couldn't build rule engine");
        let mut state = get_default_state();

        assert_eq!(re.run_when(&mut state, "connect"), Status::Accept);
    }
}
