//! vSMTP configuration

#![doc(html_no_source)]
#![deny(missing_docs)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]

/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/

/// targets for log! macro
pub mod log_channel {
    /// default log, use this instead of the root log.
    pub const DEFAULT: &str = "server";
    /// receiver system
    pub const RECEIVER: &str = "server::receiver";
    /// mail parser.
    pub const PARSER: &str = "server::parser";
    /// server's rule
    pub const SERVER_RULES: &str = "server::rule_engine";
    /// delivery system
    pub const DELIVER: &str = "server::delivery";
    /// application side's rule
    pub const USER_RULES: &str = "app::user_rules";
}

#[cfg(test)]
mod tests;

mod parser {
    pub mod semver;
    pub mod socket_addr;
    pub mod syst_group;
    pub mod syst_user;
    pub mod tls_certificate;
    pub mod tls_cipher_suite;
    pub mod tls_private_key;
    pub mod tls_protocol_version;
}

/// The configuration builder for programmatically instantiating
pub mod builder {
    mod wants;
    mod with;

    #[doc(hidden)]
    pub mod validate;
    pub use wants::*;
    pub use with::*;
}

mod log4rs_helper;
mod rustls_helper;
mod trust_dns_helper;

mod config;
mod default;

pub use config::*;
pub use log4rs_helper::get_log4rs_config;
pub use rustls_helper::get_rustls_config;
pub use trust_dns_helper::build_resolvers;

/// Re-exported dependencies
pub mod re {
    pub use log4rs;
    pub use rustls;
    // NOTE: this one should not be re-exported (because tests only)
    pub use rustls_pemfile;
    pub use users;
}

use builder::{Builder, WantsVersion};
use vsmtp_common::re::anyhow;

impl Config {
    ///
    #[must_use]
    pub const fn builder() -> Builder<WantsVersion> {
        Builder {
            state: WantsVersion(()),
        }
    }

    /// Parse a [`Config`] with [TOML] format
    ///
    /// # Errors
    ///
    /// * data is not a valid [TOML]
    /// * one field is unknown
    /// * the version requirement are not fulfilled
    /// * a mandatory field is not provided (no default value)
    ///
    /// # Panics
    ///
    /// * if the field `user` or `group` are missing, the default value `vsmtp`
    ///   will be used, if no such user/group exist, builder will panic
    ///
    /// [TOML]: https://github.com/toml-lang/toml
    pub fn from_toml(input: &str) -> anyhow::Result<Self> {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct VersionRequirement {
            #[serde(
                serialize_with = "crate::parser::semver::serialize",
                deserialize_with = "crate::parser::semver::deserialize"
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
            .map(Self::ensure)
            .map_err(anyhow::Error::new)?
    }
}
