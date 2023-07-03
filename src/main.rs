use std::{time::Instant, sync::Arc, rc::Rc, ops::Deref};

use trc::Trc;

fn test_clone_trc(n: f64) -> f64{
    let trc = Trc::new(100);
    
    let start = Instant::now();
    for _ in 0..(n as u64) {
        std::hint::black_box(trc.clone());
    }
    let end = Instant::now();
    (end-start).as_nanos() as f64 / n
}

fn test_clone_arc(n: f64) -> f64{
    let arc = Arc::new(100);
    
    let start = Instant::now();
    for _ in 0..(n as u64) {
        std::hint::black_box(arc.clone());
    }
    let end = Instant::now();
    (end-start).as_nanos() as f64 / n
}

fn test_clone_rc(n: f64) -> f64{
    let rc = Rc::new(100);
    
    let start = Instant::now();
    for _ in 0..(n as u64) {
        std::hint::black_box(rc.clone());
    }
    let end = Instant::now();
    (end-start).as_nanos() as f64 / n
}


fn test_deref_trc(n: f64) -> f64{
    let trc = Trc::new(100);
    
    let start = Instant::now();
    for _ in 0..(n as u64) {
        std::hint::black_box(trc.deref());
    }
    let end = Instant::now();
    (end-start).as_nanos() as f64 / n
}

fn test_deref_arc(n: f64) -> f64{
    let arc = Arc::new(100);
    
    let start = Instant::now();
    for _ in 0..(n as u64) {
        std::hint::black_box(arc.deref());
    }
    let end = Instant::now();
    (end-start).as_nanos() as f64 / n
}

fn test_deref_rc(n: f64) -> f64{
    let rc = Rc::new(100);
    
    let start = Instant::now();
    for _ in 0..(n as u64) {
        std::hint::black_box(rc.deref());
    }
    let end = Instant::now();
    (end-start).as_nanos() as f64 / n
}

fn main() {
    let n = 10e6;

    println!("Clone test Trc ({}x): {}ns avg", n, test_clone_trc(n));
    println!("Clone test Arc ({}x): {}ns avg", n, test_clone_arc(n));
    println!("Clone test Rc ({}x): {}ns avg", n, test_clone_rc(n));

    println!("Deref test Trc ({}x): {}ns avg", n, test_deref_trc(n));
    println!("Deref test Arc ({}x): {}ns avg", n, test_deref_arc(n));
    println!("Deref test Rc ({}x): {}ns avg", n, test_deref_rc(n));
}