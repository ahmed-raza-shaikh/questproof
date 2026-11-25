//! In-memory BM25 index over policy chunks.

use crate::corpus::Chunk;
use crate::text;
use std::collections::{HashMap, HashSet};

const K1: f32 = 1.2;
const B: f32 = 0.75;

pub struct Index {
    postings: HashMap<String, Vec<(u32, u32)>>,
    doc_len: Vec<f32>,
    avg_len: f32,
    /// Distinct terms per chunk, used for evidence coverage.
    pub chunk_terms: Vec<HashSet<String>>,
}

impl Index {
    pub fn build(chunks: &[Chunk]) -> Self {
        let mut postings: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
        let mut doc_len = Vec::with_capacity(chunks.len());
        let mut chunk_terms = Vec::with_capacity(chunks.len());
        for (i, c) in chunks.iter().enumerate() {
            // Headings are strong topical signals, so they are indexed twice.
            let terms = text::terms(&format!("{0}\n{0}\n{1}", c.heading, c.text));
            let mut tf: HashMap<&str, u32> = HashMap::new();
            for t in &terms {
                *tf.entry(t).or_default() += 1;
            }
            for (t, n) in tf {
                postings
                    .entry(t.to_string())
                    .or_default()
                    .push((i as u32, n));
            }
            doc_len.push(terms.len() as f32);
            chunk_terms.push(terms.into_iter().collect());
        }
        let avg_len = doc_len.iter().sum::<f32>() / doc_len.len().max(1) as f32;
        Self {
            postings,
            doc_len,
            avg_len,
            chunk_terms,
        }
    }

    pub fn len(&self) -> usize {
        self.doc_len.len()
    }

    pub fn is_empty(&self) -> bool {
        self.doc_len.is_empty()
    }

    /// Inverse document frequency; unseen terms get the maximum value.
    pub fn idf(&self, term: &str) -> f32 {
        let n = self.len() as f32;
        let df = self.postings.get(term).map_or(0, Vec::len) as f32;
        (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
    }

    /// Replaces query terms missing from the vocabulary with the indexed term sharing the
    /// longest prefix (at least 6 characters), e.g. `administrativ` -> `administrator`.
    pub fn expand(&self, terms: &[String]) -> Vec<String> {
        terms
            .iter()
            .map(|t| {
                if t.len() < 6 || self.postings.contains_key(t) {
                    return t.clone();
                }
                let common = |v: &str| t.bytes().zip(v.bytes()).take_while(|(a, b)| a == b).count();
                self.postings
                    .keys()
                    .map(|v| (common(v), v))
                    .filter(|(n, v)| *n >= 6 && *n * 4 >= t.len().min(v.len()) * 3)
                    .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(a.1)))
                    .map_or_else(|| t.clone(), |(_, v)| v.clone())
            })
            .collect()
    }

    /// Top `k` chunks by BM25 score, best first.
    pub fn search(&self, terms: &[String], k: usize) -> Vec<(usize, f32)> {
        let mut scores = vec![0f32; self.len()];
        let unique: HashSet<&String> = terms.iter().collect();
        for t in unique {
            let Some(list) = self.postings.get(t) else {
                continue;
            };
            let idf = self.idf(t);
            for &(doc, tf) in list {
                let tf = tf as f32;
                let norm = K1 * (1.0 - B + B * self.doc_len[doc as usize] / self.avg_len);
                scores[doc as usize] += idf * tf * (K1 + 1.0) / (tf + norm);
            }
        }
        let mut ranked: Vec<(usize, f32)> = scores
            .into_iter()
            .enumerate()
            .filter(|s| s.1 > 0.0)
            .collect();
        ranked.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked.truncate(k);
        ranked
    }

    fn weight(&self, term: &str) -> f32 {
        let w = self.idf(term);
        if text::is_scaffolding(term) {
            w * 0.2
        } else {
            w
        }
    }

    /// IDF-weighted share of query terms present in a chunk (0.0 - 1.0).
    pub fn coverage(&self, terms: &[String], chunk: usize) -> f32 {
        let unique: HashSet<&String> = terms.iter().collect();
        let total: f32 = unique.iter().map(|t| self.weight(t)).sum();
        if total == 0.0 {
            return 0.0;
        }
        let hit: f32 = unique
            .iter()
            .filter(|t| self.chunk_terms[chunk].contains(t.as_str()))
            .map(|t| self.weight(t))
            .sum();
        hit / total
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::Corpus;

    #[test]
    fn ranks_relevant_section_first() {
        let mut c = Corpus::default();
        c.add("p.md", "# Encryption\nData is encrypted at rest with AES-256.\n# Backups\nBackups run daily.\n# Access\nMFA is required.");
        let idx = Index::build(&c.chunks);
        let q = text::terms("Is data encrypted at rest?");
        let top = idx.search(&q, 3);
        assert_eq!(c.chunks[top[0].0].heading, "Encryption");
        assert!(idx.coverage(&q, top[0].0) > 0.99);
        assert!(idx.coverage(&text::terms("bug bounty"), top[0].0) < 0.01);
    }
}
