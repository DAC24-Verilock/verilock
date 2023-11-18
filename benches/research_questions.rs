use criterion::{criterion_group, criterion_main, Criterion};
use verilock::analysis;
use verilock::task;

fn research_question_1(c: &mut Criterion) {
    c.bench_function("Copy2", |b| b.iter(|| analysis::analyze(&task::VC2)));
    c.bench_function("Copy2-Defect", |b| {
        b.iter(|| analysis::analyze(&task::VC2_))
    });
    c.bench_function("Copy3", |b| b.iter(|| analysis::analyze(&task::VC3)));
    c.bench_function("Copy3-Defect", |b| {
        b.iter(|| analysis::analyze(&task::VC3_))
    });
    c.bench_function("Copy4", |b| b.iter(|| analysis::analyze(&task::VC4)));
    c.bench_function("Copy4-Defect", |b| {
        b.iter(|| analysis::analyze(&task::VC4_))
    });
    c.bench_function("CRC", |b| b.iter(|| analysis::analyze(&task::VC5)));
    c.bench_function("CRC-Defect", |b| b.iter(|| analysis::analyze(&task::VC5_)));
    c.bench_function("CRC-Env", |b| b.iter(|| analysis::analyze(&task::VC6)));
    c.bench_function("CRC-Env-Defect", |b| {
        b.iter(|| analysis::analyze(&task::VC6_))
    });
    c.bench_function("Pipeline", |b| b.iter(|| analysis::analyze(&task::VC7)));
    c.bench_function("Pipeline-Defect", |b| {
        b.iter(|| analysis::analyze(&task::VC7_))
    });
    c.bench_function("Adder", |b| b.iter(|| analysis::analyze(&task::VC8)));
    c.bench_function("Adder-Defect", |b| {
        b.iter(|| analysis::analyze(&task::VC8_))
    });
}

fn research_question_2(c: &mut Criterion) {
    c.bench_function("Gen1", |b| b.iter(|| analysis::analyze(&task::GEN1)));
    c.bench_function("Gen2", |b| b.iter(|| analysis::analyze(&task::GEN2)));
    c.bench_function("Gen3", |b| b.iter(|| analysis::analyze(&task::GEN3)));
    c.bench_function("Gen4", |b| b.iter(|| analysis::analyze(&task::GEN4)));
    c.bench_function("Gen5", |b| b.iter(|| analysis::analyze(&task::GEN5)));
    c.bench_function("Gen6", |b| b.iter(|| analysis::analyze(&task::GEN6)));
    c.bench_function("Gen7", |b| b.iter(|| analysis::analyze(&task::GEN7)));
    c.bench_function("Gen8", |b| b.iter(|| analysis::analyze(&task::GEN8)));
    c.bench_function("Gen9", |b| b.iter(|| analysis::analyze(&task::GEN9)));
    c.bench_function("Gen10", |b| b.iter(|| analysis::analyze(&task::GEN10)));
}

criterion_group!(benches, research_question_1, research_question_2);
criterion_main!(benches);
