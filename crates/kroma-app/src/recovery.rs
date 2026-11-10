//! Crash-safe recovery snapshots (autosave every 120s while dirty).

use kroma_core::Document;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const APP_ID: &str = "Kroma";

#[derive(Clone, Debug)]
pub struct RecoveryEntry {
    pub tab_id: u64,
    pub path: PathBuf,
    pub doc_name: String,
    pub modified: SystemTime,
}

fn recovery_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KROMA_RECOVERY_DIR") {
        return Some(PathBuf::from(p));
    }
    eframe::storage_dir(APP_ID).map(|p| p.join("recovery"))
}

pub fn recovery_path(tab_id: u64) -> Option<PathBuf> {
    recovery_root().map(|d| d.join(format!("{tab_id}.kroma")))
}

pub fn write(tab_id: u64, doc: &Document) -> anyhow::Result<()> {
    let Some(root) = recovery_root() else {
        return Ok(());
    };
    fs::create_dir_all(&root)?;
    let path = root.join(format!("{tab_id}.kroma"));
    kroma_core::io::save_kroma(doc, &path)?;
    Ok(())
}

pub fn delete(tab_id: u64) {
    if let Some(p) = recovery_path(tab_id) {
        let _ = fs::remove_file(p);
    }
}

pub fn delete_path(path: &Path) {
    let _ = fs::remove_file(path);
}

pub fn discard_all() {
    let Some(root) = recovery_root() else {
        return;
    };
    let Ok(read) = fs::read_dir(root) else {
        return;
    };
    for e in read.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("kroma") {
            let _ = fs::remove_file(p);
        }
    }
}

pub fn scan() -> Vec<RecoveryEntry> {
    let Some(root) = recovery_root() else {
        return Vec::new();
    };
    let Ok(read) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in read.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("kroma") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(tab_id) = stem.parse::<u64>() else {
            continue;
        };
        let modified = e
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let doc_name = kroma_core::io::open(&path)
            .map(|d| d.name)
            .unwrap_or_else(|_| format!("Recovered {tab_id}"));
        out.push(RecoveryEntry {
            tab_id,
            path,
            doc_name,
            modified,
        });
    }
    out.sort_by_key(|e| e.modified);
    out.reverse();
    out
}
