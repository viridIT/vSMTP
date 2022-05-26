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
use crate::parser::MailMimeParser;
use vsmtp_common::MailParser;

mod mime_parser {

    mod rfc2049;
    mod rfc5322;

    #[allow(non_snake_case)]
    mod allen_p__discussion_threads__1;

    mod methods;

    mod mime1;
}

fn visit_dirs(
    dir: &std::path::Path,
    cb: &dyn Fn(&std::fs::DirEntry) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry)?;
            }
        }
    }
    Ok(())
}

#[test]
fn test_parse_whole_folder() {
    visit_dirs(
        &std::path::PathBuf::from(file!())
            .parent()
            .unwrap()
            .join(std::path::PathBuf::from("mail")),
        &|entry| -> std::io::Result<()> {
            std::fs::create_dir_all("tmp/generated").unwrap();

            let mut output = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open("tmp/generated/output.txt")
                .unwrap();

            let mail = std::fs::read_to_string(entry.path())
                .map_err(|e| {
                    std::io::Write::write(
                        &mut output,
                        format!("reading failed: '{:?}' error: '{}'\n", entry.path(), e).as_bytes(),
                    )
                    .unwrap();
                    e
                })
                .unwrap();

            MailMimeParser::default()
                .parse(mail.lines().map(str::to_string).collect::<Vec<_>>())
                .map(|_| {
                    std::io::Write::write(
                        &mut output,
                        format!("parsing success '{:?}'\n", entry.path()).as_bytes(),
                    )
                    .unwrap();
                })
                .map_err(|e| panic!("parsing failed: '{:?}' error: {}", entry.path(), e))
        },
    )
    .expect("folder contain valid mail");
}
