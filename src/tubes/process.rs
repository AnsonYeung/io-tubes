use std::{
    ffi::OsStr,
    io::{self, Error, ErrorKind},
    pin::Pin,
    process::Stdio,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    process::{Child, ChildStdin, ChildStdout, Command},
};

#[derive(Debug)]
pub struct ProcessTube {
    inner: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl ProcessTube {
    pub fn new<S: AsRef<OsStr>>(program: S) -> io::Result<Self> {
        Command::new(program).try_into()
    }
}

impl TryFrom<Command> for ProcessTube {
    type Error = io::Error;

    fn try_from(mut value: Command) -> Result<Self, Self::Error> {
        value
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?
            .try_into()
    }
}

impl TryFrom<Child> for ProcessTube {
    type Error = io::Error;

    fn try_from(mut inner: Child) -> Result<Self, Self::Error> {
        let stdin = inner.stdin.take().ok_or_else(|| {
            Error::new(ErrorKind::BrokenPipe, "Unable to extract stdin from child")
        })?;
        let stdout = inner.stdout.take().ok_or_else(|| {
            Error::new(ErrorKind::BrokenPipe, "Unable to extract stdout from child")
        })?;
        Ok(ProcessTube {
            inner,
            stdin,
            stdout,
        })
    }
}

impl From<ProcessTube> for Child {
    fn from(mut tube: ProcessTube) -> Self {
        tube.inner.stdin = Some(tube.stdin);
        tube.inner.stdout = Some(tube.stdout);
        tube.inner
    }
}

impl AsyncRead for ProcessTube {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ProcessTube {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}
