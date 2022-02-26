///
/// ```sh
/// cargo test stress::listen_and_serve -- --ignored &
/// cargo test stress::send_payload -- --ignored
/// ```
///
mod listen_and_serve;
mod send_payload;
