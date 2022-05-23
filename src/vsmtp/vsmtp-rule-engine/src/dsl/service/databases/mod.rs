pub mod csv;

/// the access mode to the database.
#[derive(Debug)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

impl std::fmt::Display for AccessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Read => "O_RDONLY",
                Self::Write => "O_WRONLY",
                Self::ReadWrite => "O_RDWR",
            }
        )
    }
}

impl std::str::FromStr for AccessMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "O_RDONLY" => Ok(Self::Read),
            "O_WRONLY" => Ok(Self::Write),
            "O_RDWR" => Ok(Self::ReadWrite),
            _ => Err(()),
        }
    }
}

/// refresh rate of the database.
#[derive(Debug)]
pub enum Refresh {
    Always,
    No,
}

impl std::str::FromStr for Refresh {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "always" => Ok(Self::Always),
            "no" => Ok(Self::No),
            _ => Err(()),
        }
    }
}
