use std::fs::OpenOptions;
use std::ptr;

#[cfg(unix)]
use std::os::fd::IntoRawFd;
#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, IntoRawHandle};

pub(crate) fn with_native_stdout_suppressed<T>(operation: impl FnOnce() -> T) -> T {
    let _silencer = NativeStdoutSilencer::new();
    operation()
}

struct NativeStdoutSilencer {
    saved_fd: i32,
    null_fd: i32,
}

impl NativeStdoutSilencer {
    fn new() -> Option<Self> {
        unsafe {
            let saved_fd = dup_fd(1);
            if saved_fd < 0 {
                return None;
            }

            let Some(null_fd) = open_null_fd() else {
                close_fd(saved_fd);
                return None;
            };

            libc::fflush(ptr::null_mut());
            if dup2_fd(null_fd, 1) < 0 {
                close_fd(saved_fd);
                close_fd(null_fd);
                return None;
            }

            Some(Self { saved_fd, null_fd })
        }
    }
}

impl Drop for NativeStdoutSilencer {
    fn drop(&mut self) {
        unsafe {
            libc::fflush(ptr::null_mut());
            dup2_fd(self.saved_fd, 1);
            close_fd(self.saved_fd);
            close_fd(self.null_fd);
        }
    }
}

#[cfg(windows)]
fn open_null_fd() -> Option<i32> {
    let file = OpenOptions::new().write(true).open("NUL").ok()?;
    let handle = file.into_raw_handle();
    let fd = unsafe { libc::open_osfhandle(handle as isize, 0) };
    if fd < 0 {
        let _ = unsafe { std::fs::File::from_raw_handle(handle) };
        return None;
    }
    Some(fd)
}

#[cfg(unix)]
fn open_null_fd() -> Option<i32> {
    Some(
        OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .ok()?
            .into_raw_fd(),
    )
}

#[cfg(windows)]
unsafe fn dup_fd(fd: i32) -> i32 {
    libc::dup(fd)
}

#[cfg(unix)]
unsafe fn dup_fd(fd: i32) -> i32 {
    libc::dup(fd)
}

#[cfg(windows)]
unsafe fn dup2_fd(source_fd: i32, target_fd: i32) -> i32 {
    libc::dup2(source_fd, target_fd)
}

#[cfg(unix)]
unsafe fn dup2_fd(source_fd: i32, target_fd: i32) -> i32 {
    libc::dup2(source_fd, target_fd)
}

#[cfg(windows)]
unsafe fn close_fd(fd: i32) -> i32 {
    libc::close(fd)
}

#[cfg(unix)]
unsafe fn close_fd(fd: i32) -> i32 {
    libc::close(fd)
}
