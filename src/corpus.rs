//! Loads policy documents (Markdown or text) and splits them into citable sections.

use anyhow::Context;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Document {
    pub file: String,
    pub title: String,
    pub owner: Option<String>,
    pub last_reviewed: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    /// Stable citation id: `<file>#<section-slug>`.
    pub id: String,
    pub doc: usize,
    pub heading: String,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Corpus {
    pub docs: Vec<Document>,
    pub chunks: Vec<Chunk>,
}

const MAX_CHUNK_CHARS: usize = 1500;

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Splits optional `---` front matter (simple `key: value` lines) from the body.
fn front_matter(raw: &str) -> (Vec<(&str, &str)>, &str) {
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (Vec::new(), raw);
    };
    let Some(end) = rest.find("\n---") else {
        return (Vec::new(), raw);
    };
    let meta = rest[..end]
        .lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    let body = rest[end + 4..].trim_start_matches(['-', '\n']);
    (meta, body)
}

impl Corpus {
    pub fn load_dir(dir: &Path) -> anyhow::Result<Self> {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .with_context(|| format!("reading policy directory {}", dir.display()))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "md" || e == "txt"))
            .collect();
        paths.sort();
        let mut corpus = Corpus::default();
        for path in paths {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let file = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            corpus.add(&file, &raw);
        }
        anyhow::ensure!(
            !corpus.chunks.is_empty(),
            "no .md or .txt policies found in {}",
            dir.display()
        );
        Ok(corpus)
    }

    pub fn add(&mut self, file: &str, raw: &str) {
        let raw = raw.replace("\r\n", "\n");
        let (meta, body) = front_matter(&raw);
        let get = |k: &str| meta.iter().find(|m| m.0 == k).map(|m| m.1.to_string());
        let doc = self.docs.len();
        self.docs.push(Document {
            file: file.to_string(),
            title: get("title").unwrap_or_else(|| file.to_string()),
            owner: get("owner"),
            last_reviewed: get("last_reviewed"),
        });

        let mut sections: Vec<(String, String)> = vec![(String::new(), String::new())];
        for line in body.lines() {
            if let Some(h) = line.strip_prefix('#') {
                sections.push((h.trim_start_matches('#').trim().to_string(), String::new()));
            } else if let Some(last) = sections.last_mut() {
                last.1.push_str(line);
                last.1.push('\n');
            }
        }
        let mut used_ids = std::collections::HashSet::new();
        for (heading, text) in sections {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let base = format!(
                "{file}#{}",
                if heading.is_empty() {
                    "intro".into()
                } else {
                    slug(&heading)
                }
            );
            for (i, part) in split_long(text).into_iter().enumerate() {
                let mut id = if i == 0 {
                    base.clone()
                } else {
                    format!("{base}-{}", i + 1)
                };
                while !used_ids.insert(id.clone()) {
                    id.push('_');
                }
                self.chunks.push(Chunk {
                    id,
                    doc,
                    heading: heading.clone(),
                    text: part,
                });
            }
        }
    }
}

fn split_long(text: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    for para in text.split("\n\n") {
        let last = parts.last_mut().expect("non-empty");
        if !last.is_empty() && last.len() + para.len() > MAX_CHUNK_CHARS {
            parts.push(para.trim().to_string());
        } else {
            if !last.is_empty() {
                last.push_str("\n\n");
            }
            last.push_str(para.trim());
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_front_matter_and_sections() {
        let mut c = Corpus::default();
        c.add(
            "ac.md",
            "---\ntitle: Access Policy\nlast_reviewed: 2026-01-01\n---\n\n# Access Policy\n\n## MFA & SSO\nMFA is required.\n\n## Empty\n\n## Reviews\nQuarterly.\n",
        );
        assert_eq!(c.docs[0].title, "Access Policy");
        assert_eq!(c.docs[0].last_reviewed.as_deref(), Some("2026-01-01"));
        let ids: Vec<&str> = c.chunks.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["ac.md#mfa-sso", "ac.md#reviews"]);
    }

    #[test]
    fn splits_long_sections_with_unique_ids() {
        let mut c = Corpus::default();
        let para = "word ".repeat(200);
        c.add("x.txt", &format!("{para}\n\n{para}\n\n{para}"));
        assert!(c.chunks.len() >= 2);
        assert_eq!(c.chunks[1].id, "x.txt#intro-2");
    }
}
