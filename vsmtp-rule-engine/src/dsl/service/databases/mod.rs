pub mod csv;

/// the access mode to the database.
#[derive(Debug)]
pub enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

impl std::str::FromStr for AccessMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "readonly" => Ok(Self::Read),
            "writeonly" => Ok(Self::Write),
            "readwrite" => Ok(Self::ReadWrite),
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
