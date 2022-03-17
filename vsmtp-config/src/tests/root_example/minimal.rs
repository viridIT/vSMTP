use crate::Config;

#[test]
fn parse() {
    let toml = include_str!("../../../../examples/config/minimal.toml");
    pretty_assertions::assert_eq!(Config::from_toml(toml).unwrap(), Config::default());
}
