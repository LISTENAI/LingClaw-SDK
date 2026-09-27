use lingclaw_sdk::watch::{SourceWatch, read_source};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
static NEXT_SCRATCH_ID: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "lingclaw-watch-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_SCRATCH_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn stable_writes_and_atomic_replacement_reload_once() {
    let dir = Scratch::new();
    let path = dir.0.join("app.lua");
    std::fs::write(&path, "first").unwrap();
    let mut watcher = SourceWatch::new(path.clone(), "first".into());
    let now = Instant::now();
    assert!(watcher.poll(now).unwrap().is_none());
    std::fs::write(&path, "part").unwrap();
    assert!(watcher.poll(now).unwrap().is_none());
    std::fs::write(&path, "complete").unwrap();
    assert!(
        watcher
            .poll(now + Duration::from_millis(250))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        watcher
            .poll(now + Duration::from_millis(500))
            .unwrap()
            .as_deref(),
        Some("complete")
    );
    assert!(
        watcher
            .poll(now + Duration::from_millis(750))
            .unwrap()
            .is_none()
    );
    let replacement = dir.0.join(".save.lua");
    std::fs::write(&replacement, "replacement").unwrap();
    std::fs::rename(replacement, &path).unwrap();
    assert!(
        watcher
            .poll(now + Duration::from_secs(1))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        watcher
            .poll(now + Duration::from_millis(1250))
            .unwrap()
            .as_deref(),
        Some("replacement")
    );
}
#[test]
fn deleted_oversized_and_invalid_utf8_files_recover() {
    let dir = Scratch::new();
    let path = dir.0.join("app.lua");
    let mut watcher = SourceWatch::new(path.clone(), "old".into());
    let now = Instant::now();
    assert!(watcher.poll(now).is_err());
    std::fs::write(&path, vec![b'a'; 65537]).unwrap();
    assert!(read_source(&path).is_err());
    std::fs::write(&path, [0xff]).unwrap();
    assert!(watcher.poll(now).is_err());
    std::fs::write(&path, "recovered").unwrap();
    assert!(watcher.poll(now).unwrap().is_none());
    assert_eq!(
        watcher
            .poll(now + Duration::from_millis(250))
            .unwrap()
            .as_deref(),
        Some("recovered")
    );
}
