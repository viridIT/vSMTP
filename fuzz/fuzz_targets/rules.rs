#![no_main]
use libfuzzer_sys::fuzz_target;

use vsmtp::tests::MockRhaiEngine;

fuzz_target!(|data: &[u8]| {
    let _ = MockRhaiEngine::from_bytes(data);
});
