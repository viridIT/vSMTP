// NOTE: should be improved

#[derive(Debug)]
pub enum ParserError {
    InvalidInput,
    InvalidMail(String),
    MandatoryHeadersNotFound(String),
    BoundaryNotFound(String),
    MisplacedBoundary(String),
}

impl std::error::Error for ParserError {}

pub type ParserResult<T> = Result<T, ParserError>;

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::InvalidInput => {
                write!(f, "input is invalid")
            }
            ParserError::InvalidMail(message) => {
                write!(f, "parsing email failed: {}", message)
            }
            ParserError::MandatoryHeadersNotFound(header) => {
                write!(f, "Mandatory header '{}' not found", header)
            }
            ParserError::BoundaryNotFound(message) => {
                write!(
                    f,
                    "Boundary not found in content-type header parameters, {}",
                    message
                )
            }
            ParserError::MisplacedBoundary(message) => {
                write!(f, "Misplaced boundary in mime message, {}", message)
            }
        }
    }
}
