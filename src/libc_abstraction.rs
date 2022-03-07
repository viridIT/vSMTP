/// return type of [fork]
pub enum ForkResult {
    /// to the parent, with the pid of the child process
    Parent(libc::pid_t),
    /// to the child
    Child,
}

/// create a child process, see fork(2)
#[inline]
pub fn fork() -> anyhow::Result<ForkResult> {
    match unsafe { libc::fork() } {
        -1 => Err(anyhow::anyhow!(
            "fork: '{}'",
            std::io::Error::last_os_error()
        )),
        0 => Ok(ForkResult::Child),
        child_pid => Ok(ForkResult::Parent(child_pid)),
    }
}

/// run a program as a background process
pub fn daemon() -> anyhow::Result<ForkResult> {
    match fork()? {
        ForkResult::Parent(_) => std::process::exit(0),
        ForkResult::Child => {
            setsid()?;
            fork()
        }
    }
}

/// run a program in a new session, see setsid(2)
pub fn setsid() -> anyhow::Result<libc::pid_t> {
    match unsafe { libc::setsid() } {
        -1 => Err(anyhow::anyhow!(
            "setsid: '{}'",
            std::io::Error::last_os_error()
        )),
        res => Ok(res),
    }
}

/// set user identity, see setuid(2)
#[inline]
pub fn setuid(uid: libc::uid_t) -> anyhow::Result<i32> {
    match unsafe { libc::setuid(uid) } {
        -1 => Err(anyhow::anyhow!(
            "setuid: '{}'",
            std::io::Error::last_os_error()
        )),
        otherwise => Ok(otherwise),
    }
}

/// set group identity, see setgid(2)
#[inline]
pub fn setgid(gid: libc::gid_t) -> anyhow::Result<i32> {
    match unsafe { libc::setgid(gid) } {
        -1 => Err(anyhow::anyhow!(
            "setgid: '{}'",
            std::io::Error::last_os_error()
        )),
        otherwise => Ok(otherwise),
    }
}

/// sets user & group rights to the given file / folder.
pub fn chown_file(path: &std::path::Path, user: &users::User) -> anyhow::Result<()> {
    // log::error!("unable to setuid of user {:?}", user.name());

    // NOTE: to_string_lossy().as_bytes() isn't the right way of converting a PathBuf
    //       to a CString because it is platform independent.

    match unsafe {
        libc::chown(
            std::ffi::CString::new(path.to_string_lossy().as_bytes())?.as_ptr(),
            user.uid(),
            // FIXME: uid as gid ?
            user.uid(),
        )
    } {
        0 => Err(anyhow::anyhow!(
            "chown: '{}'",
            std::io::Error::last_os_error()
        )),
        _ => Ok(()),
    }
}
