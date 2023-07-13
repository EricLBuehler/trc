use std::{ops::Deref, rc::Rc, sync::Arc, thread};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use trc::{SharedTrc, Trc};

//cargo install cargo-criterion
//cargo criterion

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("Clone Trc", |b| b.iter(clone_trc));
    c.bench_function("Clone Arc", |b| b.iter(clone_arc));
    c.bench_function("Clone Rc", |b| b.iter(clone_rc));
    c.bench_function("Multiple clone Trc", |b| b.iter(multi_clone_trc));
    c.bench_function("Multiple clone Arc", |b| b.iter(multi_clone_arc));
    c.bench_function("Multiple clone Rc", |b| b.iter(multi_clone_rc));
    c.bench_function("Deref Trc", |b| b.iter(deref_trc));
    c.bench_function("Deref Arc", |b| b.iter(deref_arc));
    c.bench_function("Deref Rc", |b| b.iter(deref_rc));
    c.bench_function("Multiple deref Trc", |b| b.iter(multi_deref_trc));
    c.bench_function("Multiple deref Arc", |b| b.iter(multi_deref_arc));
    c.bench_function("Multiple deref Rc", |b| b.iter(multi_deref_rc));
    c.bench_function("Multiple threads Trc", |b| b.iter(multi_thread_trc));
    c.bench_function("Multiple threads Arc", |b| b.iter(multi_thread_arc));
    c.bench_function("Multiple threads Trc Medium", |b| {
        b.iter(multi_thread_trc_medium)
    });
    c.bench_function("Multiple threads Arc Medium", |b| {
        b.iter(multi_thread_arc_medium)
    });
    c.bench_function("Multiple threads Trc Long", |b| {
        b.iter(multi_thread_trc_long)
    });
    c.bench_function("Multiple threads Arc Long", |b| {
        b.iter(multi_thread_arc_long)
    });
    c.bench_function("Multiple threads Trc Super", |b| {
        b.iter(multi_thread_trc_super)
    });
    c.bench_function("Multiple threads Arc Super", |b| {
        b.iter(multi_thread_arc_super)
    });
}

fn clone_trc() {
    let trc = Trc::new(100);
    let _ = black_box(Trc::clone(&trc));
}

fn clone_arc() {
    let arc = Arc::new(100);
    let _ = black_box(Arc::clone(&arc));
}

fn clone_rc() {
    let rc = Rc::new(100);
    let _ = black_box(Rc::clone(&rc));
}

fn multi_clone_trc() {
    let trc = Trc::new(100);
    for _ in 0..100 {
        let _ = black_box(trc.clone());
    }
}

fn multi_clone_arc() {
    let arc = Arc::new(100);
    for _ in 0..100 {
        let _ = black_box(arc.clone());
    }
}

fn multi_clone_rc() {
    let rc = Rc::new(100);
    for _ in 0..100 {
        let _ = black_box(rc.clone());
    }
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

fn multi_deref_trc() {
    let trc = Trc::new(100);
    for _ in 0..100 {
        let _ = black_box(trc.deref());
    }
}

fn multi_deref_arc() {
    let arc = Arc::new(100);
    for _ in 0..100 {
        let _ = black_box(arc.deref());
    }
}

fn multi_deref_rc() {
    let rc = Rc::new(100);
    for _ in 0..100 {
        let _ = black_box(rc.deref());
    }
}

fn multi_thread_trc() {
    let trc = Trc::new(100);
    for _ in 0..100 {
        let shared = SharedTrc::from_trc(&trc);
        thread::spawn(|| {
            let trc = SharedTrc::to_trc(shared);
            let mut sum = 0;
            for _ in 0..1000 {
                let t = trc.clone();
                sum += *t;
            }
            sum
        })
        .join()
        .unwrap();
    }
}

fn multi_thread_arc() {
    let arc = Arc::new(100);
    for _ in 0..100 {
        let arc2 = arc.clone();
        thread::spawn(move || {
            let mut sum = 0;
            for _ in 0..1000 {
                let a = arc2.clone();
                sum += *a;
            }
            sum
        })
        .join()
        .unwrap();
    }
}

fn multi_thread_trc_medium() {
    let trc = Trc::new(100);
    for _ in 0..100 {
        let shared = SharedTrc::from_trc(&trc);
        thread::spawn(|| {
            let trc = SharedTrc::to_trc(shared);
            let mut sum = 0;
            for _ in 0..5000 {
                let t = trc.clone();
                sum += *t;
            }
            sum
        })
        .join()
        .unwrap();
    }
}

fn multi_thread_arc_medium() {
    let arc = Arc::new(100);
    for _ in 0..100 {
        let arc2 = arc.clone();
        thread::spawn(move || {
            let mut sum = 0;
            for _ in 0..5000 {
                let a = arc2.clone();
                sum += *a;
            }
            sum
        })
        .join()
        .unwrap();
    }
}

fn multi_thread_trc_long() {
    let trc = Trc::new(100);
    for _ in 0..100 {
        let shared = SharedTrc::from_trc(&trc);
        thread::spawn(|| {
            let trc = SharedTrc::to_trc(shared);
            let mut sum = 0;
            for _ in 0..100000 {
                let t = trc.clone();
                sum += *t;
            }
            sum
        })
        .join()
        .unwrap();
    }
}

fn multi_thread_arc_long() {
    let arc = Arc::new(100);
    for _ in 0..100 {
        let arc2 = arc.clone();
        thread::spawn(move || {
            let mut sum = 0;
            for _ in 0..100000 {
                let a = arc2.clone();
                sum += *a;
            }
            sum
        })
        .join()
        .unwrap();
    }
}

fn multi_thread_trc_super() {
    let trc = Trc::new(100);
    for _ in 0..100 {
        let shared = SharedTrc::from_trc(&trc);
        thread::spawn(|| {
            let trc = SharedTrc::to_trc(shared);
            let mut sum = 0;
            for _ in 0..500000 {
                let t = trc.clone();
                sum += *t;
            }
            sum
        })
        .join()
        .unwrap();
    }
}

fn multi_thread_arc_super() {
    let arc = Arc::new(100);
    for _ in 0..100 {
        let arc2 = arc.clone();
        thread::spawn(move || {
            let mut sum = 0;
            for _ in 0..500000 {
                let a = arc2.clone();
                sum += *a;
            }
            sum
        })
        .join()
        .unwrap();
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
