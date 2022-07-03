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

///
#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
pub enum Either<L, R> {
    ///
    Left(L),
    ///
    Right(R),
}

impl<L, R> Either<L, R> {
    /// Return the underlying type
    ///
    /// # Panics
    ///
    /// * if it was `Right`
    pub fn unwrap_left(self) -> L {
        match self {
            Either::Left(left) => left,
            Either::Right(_) => panic!("either was `right`"),
        }
    }

    /// Return the underlying type
    ///
    /// # Panics
    ///
    /// * if it was `Left`
    pub fn unwrap_right(self) -> R {
        match self {
            Either::Left(_) => panic!("either was `left`"),
            Either::Right(right) => right,
        }
    }
}
