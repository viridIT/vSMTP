///
/// ```sh
/// cargo test stress::listen_and_serve -- --ignored &
/// cargo test stress::send_payload -- --ignored
/// ```
///
mod stress {
    mod listen_and_serve_ignored_test;
    mod send_payload_ignored_test;
}
