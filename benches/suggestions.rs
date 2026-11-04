use upodesh::typo;
use std::hint::black_box;
use criterion::{Criterion, criterion_group, criterion_main};

fn symspell_bench(c: &mut Criterion) {
    c.bench_function("symspell ami", |b| b.iter(|| typo::suggest(black_box("ami"))));
    c.bench_function("symspell bangla", |b| b.iter(|| typo::suggest(black_box("বাংলা"))));
}
criterion_group!(benches, symspell_bench);
criterion_main!(benches);
