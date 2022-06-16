//! vSMTP configuration
//!
//! This module contains the configuration for the vSMTP server.
//!
//! # Configuration
//!
//! The type [`Config`] expose two methods :
//! * [`Config::builder`] to create a new configuration builder.
//! * [`Config::from_toml`] to read a configuration from a TOML file.
//!
//! # Example
//!
//! You can find examples of TOML file at <https://github.com/viridIT/vSMTP/tree/develop/examples/config>
//!
//! # Fields
//!
//! TODO!

#![doc(html_no_source)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
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
    /// application logs (rule engine).
    pub const APP: &str = "app";
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

    pub(crate) mod validate;
    pub use wants::*;
    pub use with::*;
}

mod config;
mod default;
mod ensure;
mod log4rs_helper;
mod rustls_helper;
mod trust_dns_helper;
mod virtual_tls;

pub use config::{field, Config};

pub use log4rs_helper::get_log4rs_config;
pub use rustls_helper::get_rustls_config;
pub use trust_dns_helper::{build_resolvers, Resolvers};

/// Re-exported dependencies
pub mod re {
    pub use humantime_serde::re::humantime;
    pub use log4rs;
    pub use rustls;
    // NOTE: this one should not be re-exported (because tests only)
    pub use rustls_pemfile;
    pub use users;
}

use builder::{Builder, WantsVersion};
use vsmtp_common::{libc_abstraction::chown, re::anyhow};

impl Config {
    /// Create an instance of [`Builder`].
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

#[doc(hidden)]
pub fn create_app_folder(
    config: &Config,
    path: Option<&str>,
) -> anyhow::Result<std::path::PathBuf> {
    if !config.app.dirpath.exists() {
        std::fs::create_dir_all(&config.app.dirpath)?;
    }

    let absolute_app_dirpath = config.app.dirpath.canonicalize()?;
    let full_path = path.map_or_else(
        || config.app.dirpath.clone(),
        |path| config.app.dirpath.join(path),
    );

    if !full_path.exists() {
        std::fs::create_dir_all(&full_path)?;
        chown(
            &full_path,
            Some(config.server.system.user.uid()),
            Some(config.server.system.group.gid()),
        )?;

        // NOTE: `canonicalize` cannot be used before creating folders
        //        because it checks if the result path exists or not.
        // FIXME: Even if the path is invalid (`path` parameter uses
        //        `..` or `/` to go out of the app dirpath) the folder
        //        is created anyway.
        if !full_path.canonicalize()?.starts_with(&absolute_app_dirpath) {
            anyhow::bail!("Tried to create the app folder at {:?} but the root app directory {:?} is no longer the parent. All application output must be within the app directory path specified in the toml configuration.", full_path, config.app.dirpath)
        }
    }

    Ok(full_path)
}
