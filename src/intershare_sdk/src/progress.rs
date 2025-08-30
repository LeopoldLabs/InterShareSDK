use std::io::{self, Read, Write};

pub struct ProgressWriter<W: Write, F: FnMut(u64)> {
    inner: W,
    sent: u64,
    progress_callback: F,
}

impl<W: Write, F: FnMut(u64)> ProgressWriter<W, F> {
    pub fn new(inner: W, progress_callback: F) -> Self {
        Self {
            inner,
            sent: 0,
            progress_callback,
        }
    }

    pub fn into_inner(self) -> (W, u64) {
        (self.inner, self.sent)
    }
}

impl<W: Write, F: FnMut(u64)> Write for ProgressWriter<W, F> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written_bytes = self.inner.write(buf)?;
        self.sent += written_bytes as u64;
        (self.progress_callback)(self.sent);

        return Ok(written_bytes);
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub struct ProgressReader<R: Read, F: FnMut(u64), C: Fn() -> bool> {
    inner: R,
    read: u64,
    callback: F,
    should_cancel: C,
}

impl<R: Read, F: FnMut(u64), C: Fn() -> bool> ProgressReader<R, F, C> {
    pub fn new(inner: R, callback: F, should_cancel: C) -> Self {
        Self {
            inner,
            read: 0,
            callback,
            should_cancel,
        }
    }
}

impl<R: Read, F: FnMut(u64), C: Fn() -> bool> Read for ProgressReader<R, F, C> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if (self.should_cancel)() {
            return Err(io::Error::new(io::ErrorKind::Other, "transfer cancelled"));
        }

        let read_bytes = self.inner.read(buf)?;
        self.read += read_bytes as u64;
        (self.callback)(self.read);

        Ok(read_bytes)
    }
}
