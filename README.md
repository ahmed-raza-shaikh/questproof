# compliance-autopilot

Answer security questionnaires (SIG, CAIQ, custom vendor spreadsheets) from your own policy documents. Every answer cites its source, and anything the tool can't prove goes to a human.

Security and GRC teams spend days on each customer questionnaire. `compliance-autopilot` reads your policies (Markdown or text), finds the section that answers each question, and returns:

- **An answer**: `Yes` / `No` for closed questions, with polarity detection, so "Customer data is *never* stored on personal devices" correctly answers *No*. Open questions get the exact evidence sentence.
- **A citation** to the policy section (`data-protection.md#encryption-at-rest`).
- **A confidence score**: the IDF-weighted share of the question that the evidence actually covers.
- **A status**: `auto`, `library` (reused from previously approved answers) or `review`. Answers go to `review` when evidence is weak or the cited policy is past its review date.
- **Control mapping** to SOC 2 TSC, ISO/IEC 27001:2022 Annex A and NIST CSF 2.0.
- **Optional LLM drafting** through any OpenAI-compatible API. The model sees only the retrieved excerpts, and its citations are checked against them. A hallucinated citation sends the answer to review.

Nothing leaves your machine unless you pass `--llm`.

## Quick start

```bash
cargo build --release

./target/release/compliance-autopilot answer \
  --policies examples/policies \
  --questionnaire examples/questionnaire.csv \
  --library examples/answer-library.csv \
  --format markdown
```

```
| ID      | Question                                            | Answer | Status  | Confidence | Citation                                               |
|---------|-----------------------------------------------------|--------|---------|------------|--------------------------------------------------------|
| SIG-1.1 | Is multi-factor authentication enforced for remote access? | Yes | Auto | 0.77 | access-control.md#authentication |
| SIG-2.2 | Is customer data stored on personal devices?        | No     | Auto    | 1.00       | data-protection.md#removable-media-and-personal-devices |
| SIG-4.2 | Do you operate a bug bounty program?                | Yes. We run a private bug bounty program; ... | Library | 0.75 | library: Do you have a bug bounty program? |
| SIG-5.1 | Are third-party vendors assessed before onboarding? | Yes    | Review  | 0.55       | vendor-management.md#vendor-due-diligence  (policy last reviewed 2024) |
10 questions: 7 auto-answered, 2 from library, 1 need review (90% automated)
```

The questionnaire input is any CSV with a `question` column (`id` is optional). The default output is CSV, ready to paste back into the customer's spreadsheet. `--format json` and `--format markdown` are also available.

### Find policy gaps before an audit

```bash
compliance-autopilot coverage --policies ./policies
```

The output lists each control area (access control, encryption, logging, incident response, BCP/DR, vendor management, SDLC, vulnerability management, HR security and data lifecycle). For each area it shows the best supporting section, or **GAP**, alongside the mapped SOC 2, ISO 27001 and NIST CSF controls.

### Policy front matter (optional)

```markdown
---
title: Access Control Policy
owner: Security Engineering
last_reviewed: 2026-06-15
---
```

`last_reviewed` drives stale-evidence routing (`--stale-days 365` by default).

### LLM drafting

```bash
export OPENAI_API_KEY=...            # or point OPENAI_BASE_URL at Azure OpenAI / vLLM / Ollama
compliance-autopilot answer --policies ./policies --questionnaire q.csv --llm --llm-model gpt-4o-mini
```

Retrieval confidence still gates the status. The LLM only rewrites answers it can ground in the supplied excerpts.

## Accuracy and performance

Measured with `compliance-autopilot eval --policies examples/policies` and `cargo bench`:

| Metric | Result |
|---|---|
| Precision of automatic answers (correct value **and** correct source document) | **1.000** (44/44) |
| Questions not covered by any policy that were correctly routed to review | **1.000** (7/7) |
| Automation rate on answerable questions | **0.957** (44/46) |
| Automation rate on all questions | 0.830 (44/53) |
| Answer 53 questions against 5,000 policy sections | **1.3 ms** (~24 µs per question) |
| Index 5,000 policy sections | 28 ms |

The two answerable questions sent to review use wording that doesn't appear in the policy ("Which encryption algorithm *protects*...", "Are logs *protected* against tampering?"). This is the case `--llm` drafting is meant for.

Be careful how you read these numbers. The 53-question gold set was used while tuning the tokenizer, so treat it as a development set. Before relying on the automation rate, measure it on your own policies and past questionnaires. Add your questions to `datasets/questionnaire_gold.csv` (`expected_answer` is `Yes`/`No`/`Text`/`Review`) and run `eval`.

The tool is designed to fail safe. When it isn't sure, it asks for review instead of guessing. `cargo test` enforces precision ≥ 0.99, review routing ≥ 0.99 and automation ≥ 0.90. To run the bundled GitHub Actions workflow, copy `ci/github-actions.yml` to `.github/workflows/ci.yml`.

```bash
cargo test                                               # unit + end-to-end + accuracy gate
cargo run --release -- eval --policies examples/policies # metrics and every miss
cargo bench
```

## Limitations

- Retrieval is lexical (BM25 with synonym folding and light stemming). Paraphrases with no shared vocabulary go to review unless `--llm` is used.
- Yes/No polarity looks for negations near the matched terms. Unusual phrasings such as double negatives can be misread, which is one reason `review` exists.
- Input formats are Markdown and plain text. Convert PDF or DOCX policies first (for example with `pandoc`).

## License

MIT
