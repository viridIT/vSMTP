pub mod csv;

/// the access mode to the database.
#[derive(Debug)]
pub(crate) enum AccessMode {
    Read,
    Write,
    ReadWrite,
}
