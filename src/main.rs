use std::{time::Instant, sync::Arc, rc::Rc};

use trc::Trc;

fn test_clone_trc() -> f64{
    let trc = Trc::new(());
    
    let mut sum: u128 = 0;
    for _ in 0..100000 {
        let start = Instant::now();
        let _ = Trc::clone(&trc);
        let end = Instant::now();
        sum += (end-start).as_nanos();
    }

    return sum as f64 / 100000.;
}

fn test_clone_arc() -> f64{
    let arc = Arc::new(());
    
    let mut sum: u128 = 0;
    for _ in 0..100000 {
        let start = Instant::now();
        let _ = Arc::clone(&arc);
        let end = Instant::now();
        sum += (end-start).as_nanos();
    }

    return sum as f64 / 100000.;
}

fn test_clone_rc() -> f64{
    let rc = Rc::new(());
    
    let mut sum: u128 = 0;
    for _ in 0..100000 {
        let start = Instant::now();
        let _ = Rc::clone(&rc);
        let end = Instant::now();
        sum += (end-start).as_nanos();
    }

    return sum as f64 / 100000.;
}


fn test_deref_trc() -> f64{
    let trc = Trc::new(100);
    
    let mut sum: u128 = 0;
    for _ in 0..100000 {
        let start = Instant::now();
        let _ = *trc;
        let end = Instant::now();
        sum += (end-start).as_nanos();
    }

    return sum as f64 / 100000.;
}

fn test_deref_arc() -> f64{
    let arc = Arc::new(100);
    
    let mut sum: u128 = 0;
    for _ in 0..100000 {
        let start = Instant::now();
        let _ = *arc;
        let end = Instant::now();
        sum += (end-start).as_nanos();
    }

    return sum as f64 / 100000.;
}

fn test_deref_rc() -> f64{
    let rc = Rc::new(100);
    
    let mut sum: u128 = 0;
    for _ in 0..100000 {
        let start = Instant::now();
        let _ = *rc;
        let end = Instant::now();
        sum += (end-start).as_nanos();
    }

    return sum as f64 / 100000.;
}

fn main() {
    println!("Clone test Trc (100000x): {}ns avg", test_clone_trc());
    println!("Clone test Arc (100000x): {}ns avg", test_clone_arc());
    println!("Clone test Rc (100000x): {}ns avg", test_clone_rc());

    println!("Deref test Trc (100000x): {}ns avg", test_deref_trc());
    println!("Deref test Arc (100000x): {}ns avg", test_deref_arc());
    println!("Deref test Rc (100000x): {}ns avg", test_deref_rc());
}