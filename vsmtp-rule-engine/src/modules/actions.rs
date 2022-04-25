/**
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
**/
use vsmtp_common::{mail_context::MailContext, re::anyhow};

pub mod bcc;
pub mod headers;
pub mod logging;
pub mod rule_state;
pub mod services;
pub mod transports;
pub mod utils;
pub mod write;

/// create a folder at `[app.dirpath]` if needed, or just create the app folder.
fn create_app_folder(
    config: &vsmtp_config::Config,
    path: Option<&str>,
) -> anyhow::Result<std::path::PathBuf> {
    let path = path.map_or_else(
        || config.app.dirpath.clone(),
        |path| config.app.dirpath.join(path),
    );

    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }

    Ok(path)
}

#[cfg(test)]
mod test {

    use super::create_app_folder;
    use vsmtp_common::mail_context::MailContext;
    use vsmtp_config::Config;

    pub fn get_default_context() -> MailContext {
        MailContext {
            body: vsmtp_common::mail_context::Body::Empty,
            connection_timestamp: std::time::SystemTime::now(),
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: vsmtp_common::envelop::Envelop::default(),
            metadata: Some(vsmtp_common::mail_context::MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                ..vsmtp_common::mail_context::MessageMetadata::default()
            }),
        }
    }

    #[test]
    fn test_create_app_folder() {
        let mut config = Config::default();
        config.app.dirpath = "./tests/generated".into();

        let app_folder = create_app_folder(&config, None).unwrap();
        let nested_folder = create_app_folder(&config, Some("folder")).unwrap();
        let deep_folder = create_app_folder(&config, Some("deep/folder")).unwrap();

        assert_eq!(app_folder, config.app.dirpath);
        assert!(app_folder.exists());
        assert_eq!(
            nested_folder,
            std::path::PathBuf::from_iter([config.app.dirpath.to_str().unwrap(), "folder"])
        );
        assert!(nested_folder.exists());
        assert_eq!(
            deep_folder,
            std::path::PathBuf::from_iter([config.app.dirpath.to_str().unwrap(), "deep", "folder"])
        );
        assert!(deep_folder.exists());
    }
}
