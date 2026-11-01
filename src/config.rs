//! Persistent settings (recent files, theme) and crash-recovery storage.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub recent: Vec<String>,
    #[serde(default)]
    pub light: bool,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_dir() -> PathBuf {
    let base = if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(x)
    } else if let Some(x) = std::env::var_os("APPDATA") {
        PathBuf::from(x)
    } else if cfg!(target_os = "macos") {
        home().join("Library/Application Support")
    } else {
        home().join(".config")
    };
    base.join("spritecraft")
}

pub fn cache_dir() -> PathBuf {
    let base = if let Some(x) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(x)
    } else if let Some(x) = std::env::var_os("LOCALAPPDATA") {
        PathBuf::from(x)
    } else if cfg!(target_os = "macos") {
        home().join("Library/Caches")
    } else {
        home().join(".cache")
    };
    base.join("spritecraft")
}

impl Config {
    pub fn load() -> Config {
        std::fs::read_to_string(config_dir().join("config.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dir.join("config.json"), s);
        }
    }

    pub fn add_recent(&mut self, p: &Path) {
        let s = p.to_string_lossy().to_string();
        self.recent.retain(|r| *r != s);
        self.recent.insert(0, s);
        self.recent.truncate(12);
        self.save();
    }
}

// ---------------------------------------------------------------- crash recovery
pub fn recovery_root() -> PathBuf {
    cache_dir().join("recovery")
}

pub fn session_dir(sid: &str) -> PathBuf {
    recovery_root().join(sid)
}

#[derive(Clone)]
pub struct Crashed {
    pub dir: PathBuf,
    /// (recovery file, original path if the project had been saved before)
    pub files: Vec<(PathBuf, Option<String>)>,
}

/// Finds recovery data left behind by sessions that are no longer running.
pub fn find_crashed(own: &str) -> Vec<Crashed> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(recovery_root()) else { return out };
    for e in rd.flatten() {
        let dir = e.path();
        if !dir.is_dir() || dir.file_name().map_or(false, |n| n == own) {
            continue;
        }
        let alive = std::fs::metadata(dir.join("alive")).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
        if alive.map_or(false, |a| a < Duration::from_secs(120)) {
            continue; // another instance is still running
        }
        let mut files = vec![];
        if let Ok(rd2) = std::fs::read_dir(&dir) {
            for f in rd2.flatten() {
                let p = f.path();
                if p.extension().map_or(false, |x| x == "spc") {
                    let orig = std::fs::read_to_string(p.with_extension("meta")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                    files.push((p, orig));
                }
            }
        }
        if files.is_empty() {
            let _ = std::fs::remove_dir_all(&dir);
        } else {
            files.sort();
            out.push(Crashed { dir, files });
        }
    }
    out
}
