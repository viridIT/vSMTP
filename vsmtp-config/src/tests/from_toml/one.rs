use crate::Config;

#[test]
fn parse() {
    let config = Config::from_toml(include_str!("one.toml"));
    assert!(config.is_ok(), "{:?}", config);
}
