#[cfg(test)]
mod tests;

mod parser {
    pub mod semver;
    pub mod socket_addr;
    pub mod tls_certificate;
    pub mod tls_private_key;
    pub mod tls_protocol_version;
}

mod builder {
    ///
    pub mod validate;

    ///
    pub mod wants;

    ///
    pub mod with;
}

mod config;

pub use builder::{validate, wants::*, with::*};
pub use config::Config;

impl Config {
    ///
    #[must_use]
    pub const fn builder() -> Builder<WantsVersion> {
        Builder {
            state: WantsVersion(()),
        }
    }

    /// Parse a [ServerConfig] with [TOML] format
    ///
    /// # Errors
    ///
    /// * data is not a valid [TOML]
    /// * one field is unknown
    /// * the version requirement are not fulfilled
    /// * a mandatory field is not provided (no default value)
    ///
    /// [TOML]: https://github.com/toml-lang/toml
    pub fn from_toml(input: &str) -> anyhow::Result<Self> {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct VersionRequirement {
            #[serde(
                serialize_with = "crate::next::parser::semver::serialize",
                deserialize_with = "crate::next::parser::semver::deserialize"
            )]
            version_requirement: semver::VersionReq,
        }

        let req = toml::from_str::<VersionRequirement>(input)?;
        let pkg_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

        if !req.version_requirement.matches(&pkg_version) {
            anyhow::bail!(
                "Version requirement not fulfilled: expected '{}' but got '{}'",
                req.version_requirement,
                env!("CARGO_PKG_VERSION")
            );
        }

        toml::from_str::<Self>(input)
            .map_err(anyhow::Error::new)
            .map(Builder::<WantsValidate>::ensure)
    }
}
