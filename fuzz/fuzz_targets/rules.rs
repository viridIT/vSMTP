#![no_main]
use libfuzzer_sys::fuzz_target;
use users::mock::MockUsers;

use vsmtp::rules::rule_engine::RhaiEngine;

fuzz_target!(|data: &[u8]| {
    let _ = RhaiEngine::<MockUsers>::new(data, MockUsers::with_current_uid(1000));
});
