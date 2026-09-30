//! Short-term memory for Ask AI: the last few exchanges from the past few minutes, so
//! follow-ups like "make it shorter" or "now translate that" just work. Kept in RAM only:
//! nothing is written to disk, and it's gone when Blurt quits.

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How many exchanges Tally keeps.
const MAX_TURNS: usize = 4;
/// Older than this and it's forgotten.
const TTL: Duration = Duration::from_secs(10 * 60);
/// Long texts are shortened so memory stays light.
const MAX_CHARS: usize = 600;

struct Turn {
    asked: String,
    reply: String,
    at: Instant,
}

pub struct Memory {
    turns: Mutex<VecDeque<Turn>>,
}

fn clip(s: &str) -> String {
    if s.chars().count() <= MAX_CHARS {
        s.to_string()
    } else {
        s.chars().take(MAX_CHARS).collect::<String>() + "…"
    }
}

impl Memory {
    pub fn new() -> Self {
        Self { turns: Mutex::new(VecDeque::new()) }
    }

    pub fn remember(&self, asked: &str, reply: &str) {
        let mut t = self.turns.lock();
        t.push_back(Turn { asked: clip(asked), reply: clip(reply), at: Instant::now() });
        while t.len() > MAX_TURNS {
            t.pop_front();
        }
    }

    pub fn clear(&self) {
        self.turns.lock().clear();
    }

    /// Recent exchanges as a short note for the AI, oldest first; None if nothing recent.
    pub fn recall(&self) -> Option<String> {
        let mut t = self.turns.lock();
        t.retain(|x| x.at.elapsed() < TTL);
        if t.is_empty() {
            return None;
        }
        let lines: Vec<String> = t
            .iter()
            .map(|x| format!("- They asked: {}\n  You wrote: {}", x.asked, x.reply))
            .collect();
        Some(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_last_few() {
        let m = Memory::new();
        for i in 0..6 {
            m.remember(&format!("q{i}"), &format!("a{i}"));
        }
        let r = m.recall().unwrap();
        assert!(!r.contains("q1") && r.contains("q2") && r.contains("q5"));
        m.clear();
        assert!(m.recall().is_none());
    }
}
