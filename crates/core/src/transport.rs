//! Same JSON protocol over Unix sockets or local, owner-only Windows named pipes.
use std::io;
use std::path::Path;
use tokio::io::{AsyncRead, AsyncWrite};
pub trait IoStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> IoStream for T {}
pub type Stream = Box<dyn IoStream>;

#[cfg(unix)]
pub struct Listener(tokio::net::UnixListener);
#[cfg(unix)]
impl Listener {
    pub fn bind(path: &Path) -> io::Result<Self> {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            if !meta.file_type().is_socket() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Endpoint exists and is not a socket",
                ));
            }
            if std::os::unix::net::UnixStream::connect(path).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "A Conn server already uses this endpoint",
                ));
            }
            std::fs::remove_file(path)?;
        }
        let listener = tokio::net::UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self(listener))
    }
    pub async fn accept(&mut self) -> io::Result<Stream> {
        Ok(Box::new(self.0.accept().await?.0))
    }
}
#[cfg(unix)]
pub async fn connect(path: &Path) -> io::Result<Stream> {
    Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
}

#[cfg(windows)]
pub fn pipe_name(path: &Path) -> String {
    let text = path.to_string_lossy();
    if text.starts_with(r"\\.\pipe\") {
        return text.into();
    }
    let hash = text.as_bytes().iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    format!(r"\\.\pipe\conn-{hash:016x}")
}
#[cfg(windows)]
fn create_pipe(
    name: &str,
    first: bool,
) -> io::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{
            Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
            SECURITY_ATTRIBUTES,
        },
    };
    // Owner Rights SID: only the creating object's owner receives access.
    let sddl: Vec<u16> = "D:P(A;;GA;;;OW)\0".encode_utf16().collect();
    let mut descriptor = std::ptr::null_mut();
    unsafe {
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let result = tokio::net::windows::named_pipe::ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                (&mut sa as *mut SECURITY_ATTRIBUTES).cast(),
            );
        LocalFree(descriptor);
        result
    }
}
#[cfg(windows)]
pub struct Listener {
    name: String,
    next: Option<tokio::net::windows::named_pipe::NamedPipeServer>,
}
#[cfg(windows)]
impl Listener {
    pub fn bind(path: &Path) -> io::Result<Self> {
        let name = pipe_name(path);
        let next = Some(create_pipe(&name, true)?);
        Ok(Self { name, next })
    }
    pub async fn accept(&mut self) -> io::Result<Stream> {
        let server = self
            .next
            .take()
            .ok_or_else(|| io::Error::other("Pipe listener stopped"))?;
        server.connect().await?;
        self.next = Some(create_pipe(&self.name, false)?);
        Ok(Box::new(server))
    }
}
#[cfg(windows)]
pub async fn connect(path: &Path) -> io::Result<Stream> {
    let name = pipe_name(path);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match tokio::net::windows::named_pipe::ClientOptions::new().open(&name) {
            Ok(s) => return Ok(Box::new(s)),
            Err(e) if e.raw_os_error() == Some(231) && tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await
            }
            Err(e) => return Err(e),
        }
    }
}
