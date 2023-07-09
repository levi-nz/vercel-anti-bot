use std::io::Write;
use std::sync::{Arc, Mutex};

/// A [std::io::Cursor] that has shared ownership.
pub struct SharedCursor {
    inner: Arc<Mutex<std::io::Cursor<Vec<u8>>>>
}

impl SharedCursor {
    /// Constructs a new [SharedCursor].
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(std::io::Cursor::new(Vec::new())))
        }
    }

    pub fn get_ref(&self) -> std::io::Result<Vec<u8>> {
        let lock = self.inner.lock().unwrap();
        Ok(lock.get_ref().clone())
    }
}

impl Clone for SharedCursor {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner)
        }
    }
}

impl Write for SharedCursor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut lock = self.inner.lock().unwrap();
        lock.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut lock = self.inner.lock().unwrap();
        lock.flush()
    }
}
