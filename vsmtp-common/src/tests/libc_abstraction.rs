/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use crate::libc_abstraction::{
    chown, fork, if_indextoname, if_nametoindex, setgid, setsid, setuid, ForkResult,
};

#[test]
fn test_fork() {
    assert!(fork().is_ok());
}

#[test]
fn test_setsid() {
    match fork().unwrap() {
        ForkResult::Parent(_) => (),
        ForkResult::Child => {
            // running the test in a subprocess to not pollute the other tests
            assert!(setsid().is_ok());
            assert!(setsid().is_err());
        }
    }
}

#[test]
fn test_setuid_current() {
    assert!(setuid(users::get_current_uid()).is_ok());
}

#[test]
fn test_setuid_root() {
    assert!(setuid(users::get_user_by_name("root").unwrap().uid()).is_err());
}

#[test]
fn test_setgid_current() {
    assert!(setgid(users::get_current_gid()).is_ok());
}

#[test]
fn test_setgid_root() {
    assert!(setgid(users::get_user_by_name("root").unwrap().primary_group_id()).is_err());
}

#[test]
fn test_if_indextoname() {
    assert!(if_indextoname(1).is_ok());
    assert!(if_indextoname(0).is_err());
    assert!(if_indextoname(1_000_000).is_err());
}

#[test]
fn test_if_nametoindex() {
    assert!(if_nametoindex("no_interface_named_like_that").is_err());
    assert!(if_nametoindex("no_interface_\0named_like_that").is_err());

    assert!(if_nametoindex(&if_indextoname(1).unwrap()).is_ok());
}

#[test]
fn test_chown_file() {
    let user = users::get_user_by_uid(users::get_current_uid()).unwrap();

    assert!(chown(
        std::path::Path::new("./no_such_file_exist"),
        Some(user.uid()),
        None
    )
    .is_err());

    let file_to_create = "./toto";
    let _file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(file_to_create)
        .unwrap();

    assert!(chown(std::path::Path::new(file_to_create), Some(user.uid()), None).is_ok());

    std::fs::remove_file(file_to_create).unwrap();
}
