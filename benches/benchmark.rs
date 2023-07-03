use std::{ops::Deref, rc::Rc, sync::Arc};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use trc::Trc;

//cargo install cargo-criterion
//cargo criterion

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("Clone Trc", |b| b.iter(|| clone_trc()));
    c.bench_function("Clone Arc", |b| b.iter(|| clone_arc()));
    c.bench_function("Clone Rc", |b| b.iter(|| clone_rc()));
    c.bench_function("Deref Trc", |b| b.iter(|| deref_trc()));
    c.bench_function("Deref Arc", |b| b.iter(|| deref_arc()));
    c.bench_function("Deref Rc", |b| b.iter(|| deref_rc()));
}

fn clone_trc() {
    let trc = Trc::new(100);
    let _ = black_box(trc.clone());
}

fn clone_arc() {
    let arc = Arc::new(100);
    let _ = black_box(arc.clone());
}

fn clone_rc() {
    let rc = Rc::new(100);
    let _ = black_box(rc.clone());
}

fn deref_trc() {
    let trc = Trc::new(100);
    let _ = black_box(trc.deref());
}

fn deref_arc() {
    let arc = Arc::new(100);
    let _ = black_box(arc.deref());
}

fn deref_rc() {
    let rc = Rc::new(100);
    let _ = black_box(rc.deref());
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
