use crate::libc_abstraction::{
    chown_file, fork, if_indextoname, if_nametoindex, setgid, setsid, setuid, ForkResult,
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

    assert!(chown_file(std::path::Path::new("./no_such_file_exist"), &user,).is_err());

    let file_to_create = "./toto";
    let _file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(file_to_create)
        .unwrap();

    assert!(chown_file(std::path::Path::new(file_to_create), &user,).is_ok());
}
