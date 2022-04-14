/// Return type of [fork]
pub enum ForkResult {
    /// to the parent, with the pid of the child process
    Parent(libc::pid_t),
    /// to the child
    Child,
}

/// Create a child process
///
/// # Errors
///
/// see fork(2) ERRORS
///
/// * A system-imposed limit on the number of threads was encountered
///   * the RLIMIT_NPROC soft resource limit was reached
///   * the kernel's system-wide limit on the number of processes and threads was reached
///   * the maximum number of PIDs was reached
///   * the PID limit imposed by the cgroup "process number" controller was reached.
/// * The caller is operating under the SCHED_DEADLINE scheduling policy
///   and does not have the reset-on-fork flagset.
/// * fork() is not supported on this platform
/// * fork() failed to allocate the necessary kernel structures because memory is tight
/// * An attempt was made to create a child process in a PID namespace whose "init" process has terminated
/// * System call was interrupted by a signal and will be restarted (only during a trace)
#[inline]
pub fn fork() -> anyhow::Result<ForkResult> {
    #[allow(unsafe_code)]
    match unsafe { libc::fork() } {
        // [coverage] hard to test (other than bomb fork)
        -1 => Err(anyhow::anyhow!(
            "fork: '{}'",
            std::io::Error::last_os_error()
        )),
        0 => Ok(ForkResult::Child),
        child_pid => Ok(ForkResult::Parent(child_pid)),
    }
}

/// Run a program as a background process
///
/// # Errors
///
/// see daemon(2) ERRORS
pub fn daemon(nochdir: bool, noclose: bool) -> anyhow::Result<()> {
    #[allow(unsafe_code)]
    match unsafe { libc::daemon(i32::from(nochdir), i32::from(noclose)) } {
        0 => Ok(()),
        _ => Err(anyhow::anyhow!(
            "daemon: '{}'",
            std::io::Error::last_os_error()
        )),
    }
}

/// Run a program in a new session
///
/// # Errors
///
/// see setsid(2) ERRORS
///
/// EPERM: The process group ID of any process equals the PID of the calling process.
/// Thus, in particular, setsid() fails if the calling process is already a process group leader.
pub fn setsid() -> anyhow::Result<libc::pid_t> {
    #[allow(unsafe_code)]
    match unsafe { libc::setsid() } {
        -1 => Err(anyhow::anyhow!(
            "setsid: '{}'",
            std::io::Error::last_os_error()
        )),
        res => Ok(res),
    }
}

/// Set user identity
///
/// # Errors
///
/// see setuid(2) ERRORS
#[inline]
pub fn setuid(uid: libc::uid_t) -> anyhow::Result<i32> {
    #[allow(unsafe_code)]
    match unsafe { libc::setuid(uid) } {
        -1 => Err(anyhow::anyhow!(
            "setuid: '{}'",
            std::io::Error::last_os_error()
        )),
        otherwise => Ok(otherwise),
    }
}

/// Set group identity
///
/// # Errors
///
/// see setgid(2) ERRORS
#[inline]
pub fn setgid(gid: libc::gid_t) -> anyhow::Result<i32> {
    #[allow(unsafe_code)]
    match unsafe { libc::setgid(gid) } {
        -1 => Err(anyhow::anyhow!(
            "setgid: '{}'",
            std::io::Error::last_os_error()
        )),
        otherwise => Ok(otherwise),
    }
}

/// Initialize the supplementary group access list
///
/// # Errors
///
/// see initgroups(2) ERRORS
pub fn initgroups(user: &str, gid: libc::gid_t) -> anyhow::Result<()> {
    #[allow(unsafe_code)]
    match unsafe { libc::initgroups(std::ffi::CString::new(user)?.as_ptr(), gid) } {
        0 => Ok(()),
        _ => Err(anyhow::anyhow!(
            "initgroups: '{}'",
            std::io::Error::last_os_error()
        )),
    }
}

/// Change ownership of a file
///
/// # Errors
///
/// * `@path` cannot be convert to CString
/// * see chown(2) ERRORS
pub fn chown(path: &std::path::Path, user: Option<u32>, group: Option<u32>) -> anyhow::Result<()> {
    #[allow(unsafe_code)]
    match unsafe {
        libc::chown(
            std::ffi::CString::new(path.to_string_lossy().as_bytes())?.as_ptr(),
            user.unwrap_or(u32::MAX),
            group.unwrap_or(u32::MAX),
        )
    } {
        0 => Ok(()),
        otherwise => Err(anyhow::anyhow!(
            "failed to change file owner: ({}) '{}'",
            otherwise,
            std::io::Error::last_os_error()
        )),
    }
}

/// Returns the index of the network interface corresponding to the name `@name`
///
/// # Errors
///
/// * `@name` contain an internal 0 byte
///
/// see if_nametoindex(2) ERRORS
/// * ENXIO: No index found for the @name
pub fn if_nametoindex(name: &str) -> anyhow::Result<u32> {
    #[allow(unsafe_code)]
    match unsafe { libc::if_nametoindex(std::ffi::CString::new(name)?.as_ptr()) } {
        0 => Err(anyhow::anyhow!(
            "if_nametoindex: '{}'",
            std::io::Error::last_os_error()
        )),
        otherwise => Ok(otherwise),
    }
}

/// Returns the name of the network interface corresponding to the interface `@index`
///
/// # Errors
///
/// * No interface found for the `@index`
/// * Interface name is not utf8
pub fn if_indextoname(index: u32) -> anyhow::Result<String> {
    let mut buf = [0; libc::IF_NAMESIZE];

    #[allow(unsafe_code)]
    match unsafe { libc::if_indextoname(index, buf.as_mut_ptr()) } {
        null if null.is_null() => Err(anyhow::anyhow!(
            "if_indextoname: '{}'",
            std::io::Error::last_os_error()
        )),
        _ => Ok(unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }
            .to_str()?
            .to_string()),
    }
}

/// Get user's home directory
///
/// # Errors
///
/// * see getpwuid(2) ERRORS
/// * the filepath does not contain valid utf8 data
#[allow(unsafe_code)]
pub fn getpwuid(uid: libc::uid_t) -> anyhow::Result<std::path::PathBuf> {
    let passwd = unsafe { libc::getpwuid(uid) };
    if passwd.is_null() || unsafe { *passwd }.pw_dir.is_null() {
        anyhow::bail!("getpwuid: '{}'", std::io::Error::last_os_error());
    }
    Ok(unsafe { std::ffi::CStr::from_ptr((*passwd).pw_dir) }
        .to_str()?
        .into())
}
