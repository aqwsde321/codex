use std::io::Read;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub(crate) struct BodyRecorder {
    bytes: Arc<Mutex<Vec<u8>>>,
}

impl BodyRecorder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn wrap<R>(&self, inner: R) -> RecordingReader<R> {
        RecordingReader {
            inner,
            bytes: self.bytes.clone(),
        }
    }

    pub(crate) fn bytes(&self) -> Vec<u8> {
        match self.bytes.lock() {
            Ok(bytes) => bytes.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

pub(crate) struct RecordingReader<R> {
    inner: R,
    bytes: Arc<Mutex<Vec<u8>>>,
}

impl<R: Read> Read for RecordingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buf)?;
        if read > 0 {
            match self.bytes.lock() {
                Ok(mut bytes) => bytes.extend_from_slice(&buf[..read]),
                Err(poisoned) => poisoned.into_inner().extend_from_slice(&buf[..read]),
            }
        }
        Ok(read)
    }
}

#[cfg(test)]
#[path = "body_recorder_tests.rs"]
mod tests;
