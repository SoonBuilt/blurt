//! The last few things Blurt wrote for you, so nothing is ever really lost.
//!
//! This is the safety net for the case where text is dictated with nowhere to put it:
//! open Blurt, find it, copy it. Unlike [`crate::memory`] it survives quitting, so it
//! has to live on disk — a single small JSON file next to the settings, holding at most
//! [`MAX`] entries. It can be turned off, and turning it off deletes the file.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// How many entries are kept. Ten is what people ask for and stays a few KB.
const MAX: usize = 10;
/// Long results are clipped, so one huge dictation can't bloat the file.
const MAX_CHARS: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Plain dictation.
    Dictate,
    /// An Ask AI reply.
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Milliseconds since the epoch, so the UI can say "2 minutes ago".
    pub at: u64,
    pub kind: Kind,
    /// What you said, for Ask AI entries. Empty for dictation.
    #[serde(default)]
    pub asked: String,
    /// What Blurt produced.
    pub text: String,
}

pub struct History {
    path: PathBuf,
    entries: Mutex<Vec<Entry>>,
}

fn clip(s: &str) -> String {
    if s.chars().count() <= MAX_CHARS {
        s.to_string()
    } else {
        s.chars().take(MAX_CHARS).collect::<String>() + "…"
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl History {
    /// Loads whatever is on disk. A missing or unreadable file just means no history.
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join("history.json");
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<Entry>>(&s).ok())
            .unwrap_or_default();
        Self { path, entries: Mutex::new(entries) }
    }

    pub fn list(&self) -> Vec<Entry> {
        self.entries.lock().clone()
    }

    /// Records one result, newest first, and writes the file.
    pub fn add(&self, kind: Kind, asked: &str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let entry = Entry { at: now_ms(), kind, asked: clip(asked), text: clip(text) };
        let snapshot = {
            let mut e = self.entries.lock();
            e.insert(0, entry);
            e.truncate(MAX);
            e.clone()
        };
        self.write(&snapshot);
    }

    /// Forgets everything and removes the file from disk.
    pub fn clear(&self) {
        self.entries.lock().clear();
        let _ = std::fs::remove_file(&self.path);
    }

    fn write(&self, entries: &[Entry]) {
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string(entries) {
            // Write beside the target and rename, so a crash mid-write can't leave a
            // half-written file that loses the lot.
            Ok(json) => {
                let tmp = self.path.with_extension("json.part");
                if std::fs::write(&tmp, json).is_ok() && std::fs::rename(&tmp, &self.path).is_err() {
                    let _ = std::fs::remove_file(&tmp);
                }
            }
            Err(e) => log::error!("history: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("blurt-hist-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn keeps_ten_newest_first_and_survives_a_reload() {
        let dir = temp();
        let h = History::load(&dir);
        for i in 0..14 {
            h.add(Kind::Dictate, "", &format!("entry {i}"));
        }
        let list = h.list();
        assert_eq!(list.len(), MAX);
        assert_eq!(list[0].text, "entry 13");

        // A fresh instance reads back what the last one wrote.
        let again = History::load(&dir);
        assert_eq!(again.list().len(), MAX);
        assert_eq!(again.list()[0].text, "entry 13");

        again.clear();
        assert!(History::load(&dir).list().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn blank_results_are_not_recorded() {
        let dir = temp();
        let h = History::load(&dir);
        h.add(Kind::Dictate, "", "   ");
        assert!(h.list().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
