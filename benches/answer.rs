use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use questproof::answer::{Config, Engine};
use questproof::corpus::Corpus;
use std::hint::black_box;
use std::path::Path;

/// Real example policies plus 500 synthetic documents (~5,000 sections) as background noise.
fn large_corpus() -> Corpus {
    let mut c = Corpus::load_dir(Path::new("examples/policies")).unwrap();
    let topics = [
        "network", "asset", "physical", "privacy", "mobile", "cloud", "endpoint", "risk", "audit",
        "legal",
    ];
    for d in 0..500 {
        let mut doc = format!("# Standard {d}\n");
        for s in 0..10 {
            let t = topics[(d + s) % topics.len()];
            doc.push_str(&format!(
                "## {t} control {s}\nThe {t} team reviews {t} configurations every {s} weeks and records exceptions in the {t} register. Owners approve changes to {t} baselines.\n\n"
            ));
        }
        c.add(&format!("synthetic-{d}.md"), &doc);
    }
    c
}

fn bench(c: &mut Criterion) {
    let questions: Vec<String> = std::fs::read_to_string("datasets/questionnaire_gold.csv")
        .unwrap()
        .lines()
        .skip(1)
        .filter_map(|l| l.split(',').nth(1).map(str::to_owned))
        .collect();

    let mut g = c.benchmark_group("answer");
    g.sample_size(20);
    g.bench_function("build_index_5k_sections", |b| {
        b.iter_with_setup(large_corpus, |corpus| {
            Engine::new(corpus, Vec::new(), Config::default())
        })
    });
    let engine = Engine::new(
        large_corpus(),
        Vec::new(),
        Config {
            stale_days: None,
            ..Config::default()
        },
    );
    g.throughput(Throughput::Elements(questions.len() as u64));
    g.bench_function("answer_53_questions_5k_sections", |b| {
        b.iter(|| {
            for (i, q) in questions.iter().enumerate() {
                black_box(engine.answer(&i.to_string(), q));
            }
        })
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
