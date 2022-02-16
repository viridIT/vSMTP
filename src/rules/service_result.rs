#[derive(Debug, Clone, Copy)]
pub struct ServiceResult {
    status: std::process::ExitStatus,
}

impl ServiceResult {
    pub fn new(status: std::process::ExitStatus) -> Self {
        Self { status }
    }

    pub fn has_code(&self) -> bool {
        self.get_code().is_some()
    }

    pub fn get_code(&self) -> Option<i32> {
        self.status.code()
    }

    pub fn has_signal(&self) -> bool {
        self.get_signal().is_some()
    }

    pub fn get_signal(&self) -> Option<i32> {
        std::os::unix::prelude::ExitStatusExt::signal(&self.status)
    }
}

impl std::fmt::Display for ServiceResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.status))
    }
}
