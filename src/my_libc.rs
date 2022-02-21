pub enum Fork {
    Parent(libc::pid_t),
    Child,
}

#[inline]
pub fn fork() -> anyhow::Result<Fork> {
    match unsafe { libc::fork() } {
        -1 => Err(anyhow::anyhow!(
            "fork: '{}'",
            std::io::Error::last_os_error()
        )),
        0 => Ok(Fork::Child),
        child_pid => Ok(Fork::Parent(child_pid)),
    }
}

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
