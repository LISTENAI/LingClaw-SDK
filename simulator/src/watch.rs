//! Portable, bounded polling that also sees atomic editor saves.
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub fn read_source(path: &Path) -> std::io::Result<String> {
    let mut source = String::new();
    std::fs::File::open(path)?
        .take(65537)
        .read_to_string(&mut source)?;
    if source.len() > 65536 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Lua source exceeds 64 KiB",
        ));
    }
    Ok(source)
}
pub struct SourceWatch {
    path: PathBuf,
    loaded: String,
    candidate: Option<(String, Instant)>,
}
impl SourceWatch {
    pub fn new(path: PathBuf, loaded: String) -> Self {
        Self {
            path,
            loaded,
            candidate: None,
        }
    }
    /// Publish once after the same contents remain stable for 250 ms.
    pub fn poll(&mut self, now: Instant) -> std::io::Result<Option<String>> {
        let source = match read_source(&self.path) {
            Ok(source) => source,
            Err(error) => {
                self.candidate = None;
                return Err(error);
            }
        };
        if source == self.loaded {
            self.candidate = None;
            return Ok(None);
        }
        if let Some((candidate, since)) = &self.candidate
            && candidate == &source
            && now.duration_since(*since) >= Duration::from_millis(250)
        {
            self.loaded = source.clone();
            self.candidate = None;
            return Ok(Some(source));
        }
        if self
            .candidate
            .as_ref()
            .is_none_or(|(candidate, _)| candidate != &source)
        {
            self.candidate = Some((source, now));
        }
        Ok(None)
    }
}
