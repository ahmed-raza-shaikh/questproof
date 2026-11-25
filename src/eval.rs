//! Measures automation rate and answer accuracy against a labeled questionnaire.

use crate::answer::{Engine, Status};
use anyhow::Context;
use serde::Deserialize;
use std::fmt;
use std::path::Path;

#[derive(Deserialize)]
struct GoldRow {
    id: String,
    question: String,
    /// `Yes`, `No`, `Text` (any grounded answer) or `Review` (no policy covers it).
    expected_answer: String,
    #[serde(default)]
    expected_source: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub total: usize,
    pub answerable: usize,
    pub auto: usize,
    pub auto_correct: usize,
    pub unanswerable: usize,
    pub unanswerable_routed_to_review: usize,
    pub errors: Vec<String>,
}

impl Report {
    /// Share of all questions answered without a human.
    pub fn automation_rate(&self) -> f64 {
        self.auto as f64 / self.total.max(1) as f64
    }
    /// Share of answerable questions that were answered automatically.
    pub fn answerable_automation_rate(&self) -> f64 {
        (self.auto - (self.unanswerable - self.unanswerable_routed_to_review)) as f64
            / self.answerable.max(1) as f64
    }
    /// Share of automatic answers with the right value and the right source document.
    pub fn auto_precision(&self) -> f64 {
        if self.auto == 0 {
            1.0
        } else {
            self.auto_correct as f64 / self.auto as f64
        }
    }
    /// Share of unanswerable questions correctly sent to a human.
    pub fn review_recall(&self) -> f64 {
        if self.unanswerable == 0 {
            1.0
        } else {
            self.unanswerable_routed_to_review as f64 / self.unanswerable as f64
        }
    }
}

pub fn run(engine: &Engine, gold: &Path) -> anyhow::Result<Report> {
    let mut rdr =
        csv::Reader::from_path(gold).with_context(|| format!("opening {}", gold.display()))?;
    let mut r = Report::default();
    for row in rdr.deserialize() {
        let row: GoldRow = row.context("parsing gold questionnaire")?;
        let a = engine.answer(&row.id, &row.question);
        let auto = a.status == Status::Auto;
        r.total += 1;
        if row.expected_answer == "Review" {
            r.unanswerable += 1;
            if auto {
                r.auto += 1;
                r.errors.push(format!(
                    "{}: should need review, auto-answered from {} ({:.2})",
                    row.id, a.citation, a.confidence
                ));
            } else {
                r.unanswerable_routed_to_review += 1;
            }
            continue;
        }
        r.answerable += 1;
        if !auto {
            r.errors.push(format!(
                "{}: sent to review ({:.2}, {}) [{}]",
                row.id, a.confidence, a.citation, a.notes
            ));
            continue;
        }
        r.auto += 1;
        let value_ok = row.expected_answer == "Text" || a.answer == row.expected_answer;
        let source_ok = a.citation.starts_with(&format!("{}#", row.expected_source));
        if value_ok && source_ok {
            r.auto_correct += 1;
        } else {
            r.errors.push(format!(
                "{}: expected {} from {}, got {} from {} ({:.2})",
                row.id,
                row.expected_answer,
                row.expected_source,
                a.answer,
                a.citation,
                a.confidence
            ));
        }
    }
    Ok(r)
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "| Metric | Value |\n|---|---|")?;
        writeln!(
            f,
            "| Questions | {} ({} answerable, {} not covered by policy) |",
            self.total, self.answerable, self.unanswerable
        )?;
        writeln!(
            f,
            "| Automation rate (all questions) | {:.4} |",
            self.automation_rate()
        )?;
        writeln!(
            f,
            "| Automation rate (answerable questions) | {:.4} |",
            self.answerable_automation_rate()
        )?;
        writeln!(
            f,
            "| Precision of automatic answers (value + source) | {:.4} |",
            self.auto_precision()
        )?;
        writeln!(
            f,
            "| Unanswerable questions routed to review | {:.4} |",
            self.review_recall()
        )?;
        for e in &self.errors {
            writeln!(f, "- {e}")?;
        }
        Ok(())
    }
}
