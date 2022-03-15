// #[cfg(test)]
// mod tests;

mod parser {
    pub mod semver;
    pub mod socket_addr;
    pub mod tls_certificate;
    pub mod tls_private_key;
    pub mod tls_protocol_version;
}

mod builder;
mod config;

pub use builder::{Builder, WantsVersion};
pub use config::Config;

impl Config {
    ///
    #[must_use]
    pub const fn builder() -> Builder<WantsVersion> {
        Builder {
            state: WantsVersion(()),
        }
    }
}
