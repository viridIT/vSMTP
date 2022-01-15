#[cfg(test)]
pub mod mime_parser {
    use crate::mime::parser::MailMimeParser;

    #[allow(non_snake_case)]
    mod allen_p__discussion_threads__1;

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

                let mail = std::fs::read_to_string(entry.path()).map_err(|e| {
                    std::io::Write::write(
                        &mut output,
                        format!("reading failed: '{:?}' error: '{}'\n", entry.path(), e).as_bytes(),
                    )
                    .unwrap();
                    e
                })?;

                MailMimeParser::default()
                    .parse(mail.as_bytes())
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
}