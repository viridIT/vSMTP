#[cfg(test)]
mod test {
    use v_smtp::rules::rule_engine::init;

    // internals tests.

    #[test]
    fn init_rule_engine() {
        init();
    }
}
