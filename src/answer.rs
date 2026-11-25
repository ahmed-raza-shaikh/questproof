//! Questionnaire answering: answer library reuse, evidence retrieval, polarity, confidence and review routing.

use crate::controls;
use crate::corpus::Corpus;
use crate::index::Index;
use crate::llm::{self, LlmConfig};
use crate::text;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Grounded in policy evidence with confidence above the threshold.
    Auto,
    /// Reused from a previously approved answer.
    Library,
    /// Needs a human: weak evidence, stale evidence or failed LLM validation.
    Review,
}

#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    pub id: String,
    pub question: String,
    pub answer: String,
    pub status: Status,
    pub confidence: f32,
    pub citation: String,
    pub evidence: String,
    pub controls: String,
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryEntry {
    pub question: String,
    pub answer: String,
    #[serde(skip)]
    terms: HashSet<String>,
}

pub fn load_library(path: &Path) -> anyhow::Result<Vec<LibraryEntry>> {
    let mut rdr =
        csv::Reader::from_path(path).with_context(|| format!("opening {}", path.display()))?;
    rdr.deserialize()
        .map(|r| {
            let mut e: LibraryEntry =
                r.context("library rows need `question` and `answer` columns")?;
            e.terms = text::terms(&e.question).into_iter().collect();
            Ok(e)
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Minimum evidence coverage for an automatic answer.
    pub threshold: f32,
    /// Minimum question similarity for reusing a library answer.
    pub library_threshold: f32,
    /// Evidence older than this many days is routed to review (None disables).
    pub stale_days: Option<i64>,
    pub now_days: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            threshold: 0.6,
            library_threshold: 0.75,
            stale_days: Some(365),
            now_days: 0,
        }
    }
}

pub struct Engine {
    pub corpus: Corpus,
    pub index: Index,
    library: Vec<LibraryEntry>,
    cfg: Config,
    llm: Option<LlmConfig>,
}

const YES_NO_STARTERS: [&str; 16] = [
    "is", "are", "do", "does", "did", "has", "have", "can", "will", "was", "were", "should",
    "must", "shall", "could", "would",
];

fn is_yes_no(question: &str) -> bool {
    text::words(question)
        .first()
        .is_some_and(|w| YES_NO_STARTERS.contains(&w.as_str()))
}

fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let b = text.as_bytes();
    for i in 0..b.len() {
        let end_of_sentence = matches!(b[i], b'.' | b'!' | b'?')
            && b.get(i + 1).is_none_or(|c| c.is_ascii_whitespace());
        if end_of_sentence || b[i] == b'\n' {
            let s = text[start..=i].trim();
            if s.len() > 1 {
                out.push(s);
            }
            start = i + 1;
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

/// True when a negation appears up to four words before a matched question term.
fn negated(sentence: &str, query: &HashSet<&str>) -> bool {
    let words = text::words(sentence);
    let neg: Vec<usize> = (0..words.len())
        .filter(|&i| {
            text::NEGATIONS.contains(&words[i].as_str())
                || (text::NEGATED_AUX.contains(&words[i].as_str())
                    && words.get(i + 1).is_some_and(|n| n == "t"))
        })
        .collect();
    words.iter().enumerate().any(|(j, w)| {
        text::term(w).is_some_and(|t| query.contains(t.as_str()))
            && neg.iter().any(|&i| j > i && j - i <= 4)
    })
}

impl Engine {
    pub fn new(corpus: Corpus, library: Vec<LibraryEntry>, cfg: Config) -> Self {
        let index = Index::build(&corpus.chunks);
        Self {
            corpus,
            index,
            library,
            cfg,
            llm: None,
        }
    }

    pub fn with_llm(mut self, llm: LlmConfig) -> Self {
        self.llm = Some(llm);
        self
    }

    fn library_match(&self, terms: &HashSet<String>) -> Option<(f32, &LibraryEntry)> {
        self.library
            .iter()
            .map(|e| {
                let inter = e.terms.intersection(terms).count() as f32;
                let union = e.terms.union(terms).count().max(1) as f32;
                (inter / union, e)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .filter(|(s, _)| *s >= self.cfg.library_threshold)
    }

    fn best_sentence<'a>(&self, chunk_text: &'a str, query: &HashSet<&str>) -> &'a str {
        let score = |s: &str| -> f32 {
            let terms: HashSet<String> = text::terms(s).into_iter().collect();
            query
                .iter()
                .filter(|q| terms.contains(**q))
                .map(|q| self.index.idf(q))
                .sum()
        };
        sentences(chunk_text)
            .into_iter()
            .map(|s| (score(s), s))
            .fold(
                (f32::MIN, ""),
                |best, cur| if cur.0 > best.0 { cur } else { best },
            )
            .1
    }

    pub fn answer(&self, id: &str, question: &str) -> Answer {
        let raw_terms = text::terms(question);
        let topics = controls::classify(&raw_terms);
        let controls = topics
            .iter()
            .map(|t| controls::describe(t))
            .collect::<Vec<_>>()
            .join(" / ");
        let mut out = Answer {
            id: id.to_string(),
            question: question.to_string(),
            answer: String::new(),
            status: Status::Review,
            confidence: 0.0,
            citation: String::new(),
            evidence: String::new(),
            controls,
            notes: String::new(),
        };

        let term_set: HashSet<String> = raw_terms.iter().cloned().collect();
        if let Some((sim, entry)) = self.library_match(&term_set) {
            out.answer = entry.answer.clone();
            out.status = Status::Library;
            out.confidence = sim;
            out.citation = format!("library: {}", entry.question);
            return out;
        }

        let terms = self.index.expand(&raw_terms);
        let hits = self.index.search(&terms, 3);
        let Some(&(best, _)) = hits.iter().max_by(|a, b| {
            let (ca, cb) = (
                self.index.coverage(&terms, a.0),
                self.index.coverage(&terms, b.0),
            );
            ca.total_cmp(&cb).then(a.1.total_cmp(&b.1))
        }) else {
            out.notes = "no supporting policy text found".into();
            return out;
        };

        let chunk = &self.corpus.chunks[best];
        let doc = &self.corpus.docs[chunk.doc];
        let query: HashSet<&str> = terms.iter().map(String::as_str).collect();
        let sentence = self.best_sentence(&chunk.text, &query);
        out.confidence = self.index.coverage(&terms, best);
        out.citation = chunk.id.clone();
        out.evidence = sentence.to_string();
        out.answer = if is_yes_no(question) {
            if negated(sentence, &query) {
                "No"
            } else {
                "Yes"
            }
            .to_string()
        } else {
            sentence.to_string()
        };
        let mut notes = Vec::new();
        out.status = if out.confidence >= self.cfg.threshold {
            Status::Auto
        } else {
            notes.push(format!("low evidence coverage ({:.2})", out.confidence));
            Status::Review
        };

        if let (Some(limit), Some(age)) = (
            self.cfg.stale_days,
            doc.last_reviewed
                .as_deref()
                .and_then(days_from_iso)
                .map(|d| self.cfg.now_days - d),
        ) && age > limit
        {
            notes.push(format!(
                "evidence from '{}' last reviewed {age} days ago",
                doc.title
            ));
            out.status = Status::Review;
        }

        if let Some(cfg) = &self.llm {
            let excerpts: Vec<(&str, &str)> = hits
                .iter()
                .map(|(i, _)| {
                    (
                        self.corpus.chunks[*i].id.as_str(),
                        self.corpus.chunks[*i].text.as_str(),
                    )
                })
                .collect();
            match llm::draft(cfg, question, &excerpts) {
                Ok(a) if llm::citations_valid(&a, &excerpts) => {
                    out.answer = a.answer;
                    out.evidence = a.explanation;
                    out.citation = a.citations.join("; ");
                }
                Ok(a) => {
                    notes.push(format!(
                        "LLM answer rejected ({}); kept retrieval answer",
                        a.answer
                    ));
                    out.status = Status::Review;
                }
                Err(e) => notes.push(format!("LLM unavailable: {e:#}")),
            }
        }
        out.notes = notes.join("; ");
        out
    }
}

/// Days since the Unix epoch for the `YYYY-MM-DD` prefix of a date.
pub fn days_from_iso(s: &str) -> Option<i64> {
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let m: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    Some(era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        let mut c = Corpus::default();
        c.add(
            "p.md",
            "---\nlast_reviewed: 2020-01-01\n---\n# Encryption\nAll customer data is encrypted at rest using AES-256.\n# Devices\nCustomer data is never stored on personal devices.\n# Passwords\nPasswords must be 14 characters. Passwords aren't shared.",
        );
        Engine::new(
            c,
            Vec::new(),
            Config {
                stale_days: None,
                ..Config::default()
            },
        )
    }

    #[test]
    fn answers_yes_with_citation() {
        let a = engine().answer("1", "Is customer data encrypted at rest?");
        assert_eq!((a.answer.as_str(), a.status), ("Yes", Status::Auto));
        assert_eq!(a.citation, "p.md#encryption");
        assert!(a.controls.contains("ISO27001 A.8.24"));
    }

    #[test]
    fn detects_negative_evidence() {
        let e = engine();
        assert_eq!(
            e.answer("2", "Is customer data stored on personal devices?")
                .answer,
            "No"
        );
        assert_eq!(e.answer("3", "Are passwords shared?").answer, "No");
    }

    #[test]
    fn free_text_questions_return_the_evidence_sentence() {
        let a = engine().answer("4", "What algorithm is used for encryption at rest?");
        assert_eq!(
            a.answer,
            "All customer data is encrypted at rest using AES-256."
        );
    }

    #[test]
    fn unsupported_questions_go_to_review() {
        let a = engine().answer("5", "Do you run a bug bounty program?");
        assert_eq!(a.status, Status::Review);
    }

    #[test]
    fn stale_evidence_goes_to_review() {
        let mut c = Corpus::default();
        c.add(
            "p.md",
            "---\nlast_reviewed: 2020-01-01\n---\n# Encryption\nData is encrypted at rest.",
        );
        let cfg = Config {
            now_days: days_from_iso("2026-01-01").unwrap(),
            ..Config::default()
        };
        let a = Engine::new(c, Vec::new(), cfg).answer("6", "Is data encrypted at rest?");
        assert_eq!(a.status, Status::Review);
        assert!(a.notes.contains("last reviewed"));
    }

    #[test]
    fn library_answers_are_reused() {
        let lib = vec![LibraryEntry {
            question: "Do you have a bug bounty program?".into(),
            answer: "Yes, via HackerOne.".into(),
            terms: text::terms("Do you have a bug bounty program?")
                .into_iter()
                .collect(),
        }];
        let mut c = Corpus::default();
        c.add("p.md", "# X\nUnrelated text.");
        let a =
            Engine::new(c, lib, Config::default()).answer("7", "Do you run a bug bounty program?");
        assert_eq!(
            (a.status, a.answer.as_str()),
            (Status::Library, "Yes, via HackerOne.")
        );
    }

    #[test]
    fn splits_sentences() {
        assert_eq!(
            sentences("TLS 1.2 is used. Keys rotate!\n- item"),
            ["TLS 1.2 is used.", "Keys rotate!", "- item"]
        );
    }
}
