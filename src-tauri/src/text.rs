//! Turning a raw transcript into clean text.

use once_cell::sync::Lazy;
use regex::Regex;

static NEW_PARAGRAPH: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i),?\s*\bnew paragraph\b[,.]?\s*").unwrap());
static NEW_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i),?\s*\bnew line\b[,.]?\s*").unwrap());
static SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ \t]{2,}").unwrap());
static SPACE_BEFORE_PUNCT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+([,.!?;:])").unwrap());

/// Removes filler words and applies spoken formatting commands.
pub fn clean(raw: &str) -> String {
    let mut s = remove_fillers(raw);
    s = NEW_PARAGRAPH.replace_all(&s, "\n\n").into_owned();
    s = NEW_LINE.replace_all(&s, "\n").into_owned();
    s = SPACE_BEFORE_PUNCT.replace_all(&s, "$1").into_owned();
    s = SPACES.replace_all(&s, " ").into_owned();
    let s = s
        .lines()
        .map(|l| capitalize(l.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    s.trim().to_string()
}

fn remove_fillers(raw: &str) -> String {
    let words: Vec<&str> = raw.split(' ').collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    for w in words {
        let bare: String = w
            .chars()
            .filter(|c| c.is_alphabetic())
            .collect::<String>()
            .to_lowercase();
        let is_filler = matches!(
            bare.as_str(),
            "um" | "umm" | "uh" | "uhh" | "uhm" | "erm" | "er" | "ah" | "hmm" | "mm"
        );
        if is_filler {
            // Keep sentence-ending punctuation that was attached to the filler.
            if let Some(p) = w.chars().last().filter(|c| matches!(c, '.' | '?' | '!')) {
                if let Some(prev) = out.last_mut() {
                    if !prev.ends_with(['.', '?', '!']) {
                        prev.push(p);
                    }
                }
            }
            continue;
        }
        out.push(w.to_string());
    }
    out.join(" ")
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_fillers() {
        assert_eq!(
            clean("Um, so the copy is, uh, done."),
            "So the copy is, done."
        );
        assert_eq!(clean("I think um we're good"), "I think we're good");
    }

    #[test]
    fn spoken_commands() {
        assert_eq!(clean("Hi Sara, new line thanks"), "Hi Sara\nThanks");
        assert_eq!(clean("One. New paragraph. Two."), "One.\n\nTwo.");
    }
}
