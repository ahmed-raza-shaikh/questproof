use compliance_autopilot::answer::{Config, Engine, Status, days_from_iso, load_library};
use compliance_autopilot::corpus::Corpus;
use compliance_autopilot::eval;
use std::path::Path;

fn engine(library: bool) -> Engine {
    let corpus = Corpus::load_dir(Path::new("examples/policies")).unwrap();
    let lib = if library {
        load_library(Path::new("examples/answer-library.csv")).unwrap()
    } else {
        Vec::new()
    };
    let cfg = Config {
        now_days: days_from_iso("2026-11-25").unwrap(),
        ..Config::default()
    };
    Engine::new(corpus, lib, cfg)
}

#[test]
fn example_questionnaire_end_to_end() {
    let e = engine(true);
    let a = e.answer("SIG-2.2", "Is customer data stored on personal devices?");
    assert_eq!((a.answer.as_str(), a.status), ("No", Status::Auto));
    assert_eq!(
        a.citation,
        "data-protection.md#removable-media-and-personal-devices"
    );

    let lib = e.answer("SIG-4.2", "Do you operate a bug bounty program?");
    assert_eq!(lib.status, Status::Library);

    // The vendor policy was last reviewed in 2024, so its answers need a human.
    let stale = e.answer(
        "SIG-5.1",
        "Are third-party vendors assessed before onboarding?",
    );
    assert_eq!(stale.status, Status::Review);
    assert!(stale.notes.contains("last reviewed"));
}

#[test]
fn every_answer_has_controls_or_review() {
    let e = engine(false);
    let a = e.answer("x", "Is data encrypted in transit using TLS?");
    assert!(a.controls.contains("SOC2 CC6.1, CC6.7"));
}

#[test]
fn accuracy_gate() {
    let corpus = Corpus::load_dir(Path::new("examples/policies")).unwrap();
    let e = Engine::new(
        corpus,
        Vec::new(),
        Config {
            stale_days: None,
            ..Config::default()
        },
    );
    let r = eval::run(&e, Path::new("datasets/questionnaire_gold.csv")).unwrap();
    assert!(r.auto_precision() >= 0.99, "{r}");
    assert!(r.review_recall() >= 0.99, "{r}");
    assert!(r.answerable_automation_rate() >= 0.9, "{r}");
}
