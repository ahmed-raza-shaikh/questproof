//! Tokenization shared by indexing and querying: lowercase, synonym folding, stop words, light stemming.

const STOPWORDS: &[&str] = &[
    "a", "about", "across", "after", "against", "all", "an", "and", "any", "are", "as", "at", "be",
    "been", "before", "between", "by", "can", "do", "does", "each", "for", "from", "has", "have",
    "how", "i", "if", "in", "into", "is", "it", "its", "may", "me", "my", "of", "on", "only", "or",
    "other", "our", "ours", "over", "per", "s", "should", "such", "t", "than", "that", "the",
    "their", "them", "there", "these", "they", "this", "those", "through", "to", "under", "us",
    "via", "was", "we", "were", "what", "when", "where", "which", "who", "will", "with", "within",
    "you", "your", "yours",
];

/// Words that describe the *shape* of the requested answer ("minimum", "how often") rather
/// than its subject. They are kept for ranking but carry little weight in evidence coverage.
const SCAFFOLDING: &[&str] = &[
    "algorithm",
    "approach",
    "describe",
    "documented",
    "duration",
    "explain",
    "formal",
    "formally",
    "frequency",
    "frequently",
    "kind",
    "length",
    "list",
    "maximum",
    "mechanism",
    "method",
    "minimum",
    "often",
    "period",
    "please",
    "procedure",
    "process",
    "provide",
    "scheme",
    "specific",
    "timeframe",
    "timeline",
    "timelines",
    "type",
];

static SCAFFOLDING_TERMS: std::sync::LazyLock<std::collections::HashSet<String>> =
    std::sync::LazyLock::new(|| SCAFFOLDING.iter().map(|w| stem(synonym(w))).collect());

pub fn is_scaffolding(term: &str) -> bool {
    SCAFFOLDING_TERMS.contains(term)
}

/// Words that flip the meaning of a nearby policy statement.
pub const NEGATIONS: [&str; 6] = ["not", "no", "never", "cannot", "neither", "nor"];

/// Contraction stems that form a negation with a following `t` (e.g. "doesn't").
pub const NEGATED_AUX: [&str; 11] = [
    "don", "doesn", "isn", "aren", "won", "wasn", "weren", "haven", "hasn", "shouldn", "mustn",
];

/// Multi-word phrases folded into a single canonical token before splitting.
const PHRASES: [(&str, &str); 10] = [
    ("multi-factor", "mfa"),
    ("multi factor", "mfa"),
    ("two-factor", "mfa"),
    ("two factor", "mfa"),
    ("single sign-on", "sso"),
    ("single sign on", "sso"),
    ("penetration test", "pentest"),
    ("ci/cd", "cicd"),
    ("point-in-time", "pitr"),
    ("24/7", "24x7"),
];

fn synonym(word: &str) -> &str {
    match word {
        "2fa" | "multifactor" | "twofactor" => "mfa",
        "ssl" | "https" => "tls",
        "pentests" | "pentesting" => "pentest",
        "vulnerability" | "vulnerabilities" | "vulns" => "vuln",
        "vendor" | "vendors" | "supplier" | "suppliers" | "subprocessor" | "subprocessors" => {
            "vendor"
        }
        "staff" | "employee" | "employees" | "personnel" | "workforce" => "employee",
        "sast" => "static",
        w => w,
    }
}

pub fn is_stopword(w: &str) -> bool {
    STOPWORDS.binary_search(&w).is_ok()
}

/// Porter-lite suffix stripping; enough to merge encrypt/encrypted/encryption, log/logs/logging.
pub fn stem(word: &str) -> String {
    const SUFFIXES: [&str; 11] = [
        "ments", "ment", "ings", "ing", "ions", "ion", "ies", "ed", "es", "ly", "s",
    ];
    let mut w = word.to_string();
    for suf in SUFFIXES {
        if w.len() >= suf.len() + 3 && w.ends_with(suf) && !(suf == "s" && w.ends_with("ss")) {
            w.truncate(w.len() - suf.len());
            if suf == "ies" || (matches!(suf, "ed" | "es") && w.ends_with('i')) {
                // notified / notifies -> notify, policies -> policy
                if w.ends_with('i') {
                    w.pop();
                }
                w.push('y');
            }
            if matches!(suf, "ing" | "ings" | "ed") {
                let b = w.as_bytes();
                let n = b.len();
                if n >= 2 && b[n - 1] == b[n - 2] && !matches!(b[n - 1], b'l' | b's' | b'z') {
                    w.pop();
                }
            }
            break;
        }
    }
    if w.len() > 4 && w.ends_with('e') {
        w.pop();
    }
    w
}

/// Lowercased raw words (alphanumeric runs) after phrase folding, keeping stop words.
pub fn words(text: &str) -> Vec<String> {
    let mut lower = text.to_lowercase();
    for (phrase, token) in PHRASES {
        if lower.contains(phrase) {
            lower = lower.replace(phrase, token);
        }
    }
    lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Canonical search term for a raw word, or `None` for stop words and negations.
pub fn term(word: &str) -> Option<String> {
    let w = synonym(word);
    (!is_stopword(w) && !NEGATIONS.contains(&w)).then(|| stem(w))
}

pub fn terms(text: &str) -> Vec<String> {
    words(text).iter().filter_map(|w| term(w)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopwords_are_sorted_for_binary_search() {
        assert!(STOPWORDS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn stemming_merges_variants() {
        for group in [
            ["encrypt", "encrypted", "encryption"],
            ["log", "logs", "logging"],
            ["rotate", "rotated", "rotation"],
            ["policy", "policies", "policy"],
            ["require", "required", "requires"],
            ["notify", "notified", "notifies"],
        ] {
            let stems: Vec<String> = group.iter().map(|w| stem(w)).collect();
            assert!(
                stems.windows(2).all(|p| p[0] == p[1]),
                "{group:?} -> {stems:?}"
            );
        }
        assert_eq!(stem("access"), "access");
        assert_eq!(stem("accessed"), "access");
    }

    #[test]
    fn phrases_and_synonyms_fold() {
        assert_eq!(
            terms("Multi-Factor Authentication"),
            terms("MFA authentication")
        );
        assert_eq!(
            terms("Penetration testing by vendors"),
            ["pentest", "vendor"]
        );
        assert!(terms("is it not the").is_empty());
    }
}
