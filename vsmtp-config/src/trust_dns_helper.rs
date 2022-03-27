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

use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    error::ResolveError,
    TokioAsyncResolver,
};

use crate::Config;

/// build an async dns using tokio & trust_dns from configuration.
pub fn build_dns(config: &Config) -> Result<TokioAsyncResolver, ResolveError> {
    match &config.server.dns {
        crate::config::ConfigServerDNS::Google => {
            TokioAsyncResolver::tokio(ResolverConfig::google(), ResolverOpts::default())
        }

        crate::config::ConfigServerDNS::CloudFlare => {
            TokioAsyncResolver::tokio(ResolverConfig::cloudflare(), ResolverOpts::default())
        }
        crate::config::ConfigServerDNS::System => TokioAsyncResolver::tokio_from_system_conf(),
        crate::config::ConfigServerDNS::Custom { config, options } => {
            TokioAsyncResolver::tokio(config.clone(), *options)
        }
    }
}
