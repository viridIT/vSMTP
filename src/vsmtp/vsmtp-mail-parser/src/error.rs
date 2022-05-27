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
// NOTE: should be improved

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq)]
pub enum ParserError {
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
            Self::InvalidMail(message) => {
                write!(f, "parsing email failed: {}", message)
            }
            Self::MandatoryHeadersNotFound(header) => {
                write!(f, "Mandatory header '{}' not found", header)
            }
            Self::BoundaryNotFound(message) => {
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
