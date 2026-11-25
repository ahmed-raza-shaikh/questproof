use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use compliance_autopilot::answer::{self, Answer, Config, Engine, Status, days_from_iso};
use compliance_autopilot::corpus::Corpus;
use compliance_autopilot::{controls, eval, llm};
use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(
    version,
    about = "Answer security questionnaires from your own policies, with citations"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Csv,
    Json,
    Markdown,
}

#[derive(clap::Args)]
struct EngineArgs {
    /// Directory of policy documents (.md / .txt).
    #[arg(long)]
    policies: PathBuf,
    /// Minimum evidence coverage (0-1) for an automatic answer.
    #[arg(long, default_value_t = 0.6)]
    threshold: f32,
}

#[derive(Subcommand)]
enum Cmd {
    /// Fill in a questionnaire CSV (needs a `question` column; `id` optional).
    Answer {
        #[command(flatten)]
        engine: EngineArgs,
        #[arg(long)]
        questionnaire: PathBuf,
        /// CSV of previously approved answers (`question`,`answer`).
        #[arg(long)]
        library: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "csv")]
        format: Format,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Route answers citing policies not reviewed within this many days to a human.
        #[arg(long, default_value_t = 365)]
        stale_days: i64,
        /// Reference date (YYYY-MM-DD); defaults to today.
        #[arg(long)]
        now: Option<String>,
        /// Draft answers with an LLM (OpenAI-compatible); citations are verified.
        #[arg(long)]
        llm: bool,
        #[arg(
            long,
            env = "OPENAI_BASE_URL",
            default_value = "https://api.openai.com/v1"
        )]
        llm_base_url: String,
        #[arg(long, env = "LLM_MODEL", default_value = "gpt-4o-mini")]
        llm_model: String,
    },
    /// Show which control areas your policies cover and where the gaps are.
    Coverage {
        #[command(flatten)]
        engine: EngineArgs,
    },
    /// Measure automation and accuracy on a labeled questionnaire.
    Eval {
        #[command(flatten)]
        engine: EngineArgs,
        #[arg(long, default_value = "datasets/questionnaire_gold.csv")]
        gold: PathBuf,
        #[arg(long, default_value_t = 0.99)]
        min_precision: f64,
    },
}

#[derive(Deserialize)]
struct QuestionRow {
    #[serde(default)]
    id: String,
    question: String,
}

fn today() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64 / 86_400)
}

fn markdown(answers: &[Answer]) -> String {
    let mut s = String::from(
        "| ID | Question | Answer | Status | Confidence | Citation |\n|---|---|---|---|---|---|\n",
    );
    for a in answers {
        let cell = |v: &str| v.replace('|', "\\|").replace('\n', " ");
        s.push_str(&format!(
            "| {} | {} | {} | {:?} | {:.2} | {} |\n",
            a.id,
            cell(&a.question),
            cell(&a.answer),
            a.status,
            a.confidence,
            cell(&a.citation)
        ));
    }
    s
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().cmd {
        Cmd::Answer {
            engine,
            questionnaire,
            library,
            format,
            out,
            stale_days,
            now,
            llm,
            llm_base_url,
            llm_model,
        } => {
            let now_days = match now {
                Some(d) => days_from_iso(&d).context("--now must be YYYY-MM-DD")?,
                None => today(),
            };
            let cfg = Config {
                threshold: engine.threshold,
                stale_days: Some(stale_days),
                now_days,
                ..Config::default()
            };
            let library = library
                .map(|p| answer::load_library(&p))
                .transpose()?
                .unwrap_or_default();
            let mut eng = Engine::new(Corpus::load_dir(&engine.policies)?, library, cfg);
            if llm {
                eng = eng.with_llm(llm::LlmConfig {
                    base_url: llm_base_url,
                    api_key: std::env::var("OPENAI_API_KEY")
                        .context("--llm requires OPENAI_API_KEY")?,
                    model: llm_model,
                });
            }
            let mut rdr = csv::Reader::from_path(&questionnaire)
                .with_context(|| format!("opening {}", questionnaire.display()))?;
            let mut answers = Vec::new();
            for (i, row) in rdr.deserialize().enumerate() {
                let row: QuestionRow =
                    row.context("questionnaire rows need a `question` column")?;
                let id = if row.id.is_empty() {
                    (i + 1).to_string()
                } else {
                    row.id
                };
                answers.push(eng.answer(&id, &row.question));
            }

            let mut sink: Box<dyn Write> = match &out {
                Some(p) => Box::new(
                    std::fs::File::create(p)
                        .with_context(|| format!("creating {}", p.display()))?,
                ),
                None => Box::new(std::io::stdout().lock()),
            };
            match format {
                Format::Json => writeln!(sink, "{}", serde_json::to_string_pretty(&answers)?)?,
                Format::Markdown => write!(sink, "{}", markdown(&answers))?,
                Format::Csv => {
                    let mut w = csv::Writer::from_writer(sink);
                    for a in &answers {
                        w.serialize(a)?;
                    }
                    w.flush()?;
                }
            }
            let count = |s| answers.iter().filter(|a| a.status == s).count();
            eprintln!(
                "{} questions: {} auto-answered, {} from library, {} need review ({:.0}% automated)",
                answers.len(),
                count(Status::Auto),
                count(Status::Library),
                count(Status::Review),
                100.0 * (count(Status::Auto) + count(Status::Library)) as f64
                    / answers.len().max(1) as f64
            );
        }
        Cmd::Coverage { engine } => {
            let eng = Engine::new(
                Corpus::load_dir(&engine.policies)?,
                Vec::new(),
                Config::default(),
            );
            println!("| Control area | Covered | Best evidence | Frameworks |\n|---|---|---|---|");
            for (i, topic) in controls::TOPICS.iter().enumerate() {
                let terms: Vec<String> = controls::topic_terms(i).iter().cloned().collect();
                let best = eng.index.search(&terms, 1).first().map(|h| h.0);
                let hits = best.map_or(0, |c| {
                    controls::topic_terms(i)
                        .iter()
                        .filter(|t| eng.index.chunk_terms[c].contains(*t))
                        .count()
                });
                let covered = hits >= 2;
                println!(
                    "| {} | {} | {} | {} |",
                    topic.id,
                    if covered { "yes" } else { "**GAP**" },
                    best.filter(|_| covered)
                        .map_or("-", |c| eng.corpus.chunks[c].id.as_str()),
                    controls::describe(topic)
                );
            }
        }
        Cmd::Eval {
            engine,
            gold,
            min_precision,
        } => {
            let cfg = Config {
                threshold: engine.threshold,
                stale_days: None,
                ..Config::default()
            };
            let eng = Engine::new(Corpus::load_dir(&engine.policies)?, Vec::new(), cfg);
            let r = eval::run(&eng, &gold)?;
            println!("{r}");
            anyhow::ensure!(
                r.auto_precision() >= min_precision,
                "precision {:.4} below {min_precision}",
                r.auto_precision()
            );
        }
    }
    Ok(())
}
