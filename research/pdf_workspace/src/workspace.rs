//! Local replacement for Jobs' viewer-state autosave. One bounded writer.
use crate::{capture::Workspace, viewer};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
};
pub struct Saver {
    tx: Option<mpsc::SyncSender<Workspace>>,
    thread: Option<JoinHandle<()>>,
    path: PathBuf,
    pub error: Arc<Mutex<Option<String>>>,
}
impl Saver {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = mpsc::sync_channel::<Workspace>(1);
        let error = Arc::new(Mutex::new(None));
        let state_error = error.clone();
        let target = path.clone();
        let thread = std::thread::spawn(move || {
            let mut previous = Vec::new();
            while let Ok(state) = rx.recv() {
                match serde_json::to_vec(&state)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        let digest = Sha256::digest(&bytes).to_vec();
                        if digest != previous {
                            write(&target, &bytes)?;
                            previous = digest;
                        }
                        Ok(())
                    }) {
                    Ok(()) => {}
                    Err(e) => *state_error.lock().unwrap() = Some(e),
                }
            }
        });
        Self {
            tx: Some(tx),
            thread: Some(thread),
            path,
            error,
        }
    }
    pub fn submit(&self, state: Workspace) {
        if let Some(tx) = &self.tx {
            let _ = tx.try_send(state);
        }
    }
    pub fn finish(mut self, state: Workspace) -> Result<(), String> {
        self.tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        write(
            &self.path,
            &serde_json::to_vec(&state).map_err(|e| e.to_string())?,
        )
    }
}
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Workspace exceeds 64 MiB; previous saved state retained".into());
    }
    std::fs::create_dir_all(path.parent().ok_or("Workspace folder unavailable")?)
        .map_err(|e| e.to_string())?;
    viewer::atomic_export(path, bytes)
}
pub fn path() -> Result<PathBuf, String> {
    Ok(
        PathBuf::from(std::env::var_os("HOME").ok_or("Home folder unavailable")?)
            .join("Library/Application Support/iQsDrawingViewer/viewer-workspace.json"),
    )
}
pub fn load(path: &Path) -> Result<Option<Workspace>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Workspace exceeds 64 MiB".into());
    }
    let state: Workspace = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if state.version != 1 || !state.valid() {
        return Err("Unsupported saved workspace version".into());
    }
    Ok(Some(state))
}
