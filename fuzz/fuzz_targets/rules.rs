#![no_main]
use libfuzzer_sys::fuzz_target;
use users::mock::MockUsers;

fuzz_target!(|data: &[u8]| { todo!() });
