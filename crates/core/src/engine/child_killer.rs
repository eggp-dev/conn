//! Keep a termination handle independent of the child wait thread.

#[cfg(not(windows))]
pub(super) struct ChildKiller(Box<dyn portable_pty::ChildKiller + Send + Sync>);

#[cfg(not(windows))]
impl ChildKiller {
    pub(super) fn new(child: &dyn portable_pty::Child) -> std::io::Result<Self> {
        Ok(Self(child.clone_killer()))
    }

    pub(super) fn kill(&mut self) -> std::io::Result<()> {
        self.0.kill()
    }
}

#[cfg(all(windows, test))]
mod tests {
    use super::ChildKiller;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows_sys::Win32::{
        Foundation::ERROR_ACCESS_DENIED,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE},
    };

    #[test]
    fn termination_preserves_access_denied_for_a_running_process() {
        // A wait-only handle cannot terminate even our own process. This tests
        // a genuine Windows failure without granting termination permission.
        let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, std::process::id()) };
        assert!(!raw.is_null());
        // SAFETY: OpenProcess transferred one valid handle to this test.
        let mut killer = ChildKiller {
            handle: unsafe { OwnedHandle::from_raw_handle(raw) },
            termination_requested: false,
        };
        let error = killer.kill().unwrap_err();
        assert_eq!(error.raw_os_error(), Some(ERROR_ACCESS_DENIED as i32));
        assert!(!killer.termination_requested);
    }
}

#[cfg(windows)]
pub(super) struct ChildKiller {
    handle: std::os::windows::io::OwnedHandle,
    termination_requested: bool,
}

#[cfg(windows)]
impl ChildKiller {
    pub(super) fn new(child: &dyn portable_pty::Child) -> std::io::Result<Self> {
        use std::os::windows::io::BorrowedHandle;

        let raw = child.as_raw_handle().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Unsupported, "PTY child has no process handle")
        })?;
        // SAFETY: child owns this handle and remains alive until duplication
        // completes. OwnedHandle then keeps it valid independently of wait().
        let borrowed = unsafe { BorrowedHandle::borrow_raw(raw) };
        Ok(Self {
            handle: borrowed.try_clone_to_owned()?,
            termination_requested: false,
        })
    }

    pub(super) fn kill(&mut self) -> std::io::Result<()> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::WAIT_OBJECT_0,
            System::Threading::{TerminateProcess, WaitForSingleObject},
        };

        // TerminateProcess is asynchronous. A second call can fail with access
        // denied while the first is still completing, before the handle signals.
        if self.termination_requested {
            return Ok(());
        }

        // portable-pty 0.8.1 (also 0.9.0) inverts TerminateProcess's BOOL
        // result in clone_killer(): a successful termination becomes an error.
        // Use the owned process handle directly until a fixed release exists.
        let handle = self.handle.as_raw_handle();
        // SAFETY: this is a live, owned process handle duplicated at spawn.
        if unsafe { TerminateProcess(handle, 1) } != 0 {
            self.termination_requested = true;
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        // The shell may have exited between Engine::has_exited() and this
        // call. Only treat a signaled process as success; preserve real errors.
        // SAFETY: the same owned handle stays valid for this nonblocking wait.
        if unsafe { WaitForSingleObject(handle, 0) } == WAIT_OBJECT_0 {
            self.termination_requested = true;
            Ok(())
        } else {
            Err(error)
        }
    }
}
