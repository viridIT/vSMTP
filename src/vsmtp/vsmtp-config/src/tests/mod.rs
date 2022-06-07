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
mod root_example {
    mod antivirus;
    mod logging;
    mod minimal;
    mod secured;
    mod simple;
    mod tls;
}

mod validate;

#[test]
#[ignore]
fn test_create_app_folder() {
    let mut config = crate::Config::default();
    config.app.dirpath = "./tests/generated".into();

    let app_folder = crate::create_app_folder(&config, None).unwrap();
    let nested_folder = crate::create_app_folder(&config, Some("folder")).unwrap();
    let deep_folder = crate::create_app_folder(&config, Some("deep/folder")).unwrap();

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
