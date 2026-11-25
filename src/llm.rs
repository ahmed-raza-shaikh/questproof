//! Optional grounded answer drafting through any OpenAI-compatible chat completions API.
//! The model only sees retrieved policy excerpts, and its citations are verified afterwards.

use anyhow::Context;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Deserialize)]
pub struct LlmAnswer {
    pub answer: String,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub citations: Vec<String>,
}

pub const INSUFFICIENT: &str = "Insufficient evidence";

const SYSTEM_PROMPT: &str = r#"You complete enterprise security questionnaires.
Use ONLY the policy excerpts provided. Reply with a JSON object:
{"answer": "Yes" | "No" | "Partial" | "<one short sentence>", "explanation": "<1-2 sentences quoting the policy>", "citations": ["<excerpt id>", ...]}
If the excerpts do not clearly answer the question reply {"answer": "Insufficient evidence", "explanation": "", "citations": []}.
Never use outside knowledge and never invent excerpt ids."#;

pub fn draft(
    cfg: &LlmConfig,
    question: &str,
    excerpts: &[(&str, &str)],
) -> anyhow::Result<LlmAnswer> {
    let context: String = excerpts
        .iter()
        .map(|(id, text)| format!("[{id}]\n{text}\n"))
        .collect();
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let resp: Value = ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", cfg.api_key))
        .timeout(Duration::from_secs(60))
        .send_json(json!({
            "model": cfg.model,
            "temperature": 0,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user", "content": format!("Policy excerpts:\n{context}\nQuestion: {question}") }
            ]
        }))
        .context("calling LLM endpoint")?
        .into_json()
        .context("decoding LLM response")?;
    let content = resp["choices"][0]["message"]["content"]
        .as_str()
        .context("LLM response had no content")?;
    serde_json::from_str(content).context("LLM did not return the expected JSON")
}

/// True when every citation refers to an excerpt the model was actually given.
pub fn citations_valid(a: &LlmAnswer, excerpts: &[(&str, &str)]) -> bool {
    a.answer != INSUFFICIENT
        && !a.citations.is_empty()
        && a.citations
            .iter()
            .all(|c| excerpts.iter().any(|(id, _)| id == c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_hallucinated_citations() {
        let ex = [("a.md#mfa", "MFA is required.")];
        let ok = LlmAnswer {
            answer: "Yes".into(),
            explanation: String::new(),
            citations: vec!["a.md#mfa".into()],
        };
        let bad = LlmAnswer {
            citations: vec!["b.md#x".into()],
            ..LlmAnswer {
                answer: "Yes".into(),
                explanation: String::new(),
                citations: vec![],
            }
        };
        let none = LlmAnswer {
            answer: INSUFFICIENT.into(),
            explanation: String::new(),
            citations: vec![],
        };
        assert!(citations_valid(&ok, &ex));
        assert!(!citations_valid(&bad, &ex));
        assert!(!citations_valid(&none, &ex));
    }
}
