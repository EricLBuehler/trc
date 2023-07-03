use std::thread;

use crate::{
    trc::{SharedTrc, Trc},
    Weak,
};

struct Data {
    string: String,
    int: i32,
}

#[test]
fn test_singlethreaded() {
    let data = Data {
        string: String::from("This is data."),
        int: 123,
    };

    let mut trc = Trc::new(data);
    println!("Deref test! {}", trc.int);
    println!("DerefMut test");
    (*unsafe { Trc::deref_mut(&mut trc) }).string = String::from("This is also data");
    println!("Deref test! {}", trc.string);
}

#[test]
fn test_singlethreaded2() {
    let mut trc = Trc::new(100);
    assert_eq!(*trc, 100);
    *unsafe { Trc::deref_mut(&mut trc)} = 200;
    assert_eq!(*trc, 200);
}

#[test]
fn test_refcount() {
    let trc = Trc::new(100);
    let alt = trc.clone();
    println!();
    println!("localref {}", Trc::local_refcount(&trc));
    println!("atomicref {}", Trc::atomic_count(&trc));
    let _shared = SharedTrc::from_trc(&trc);
    println!("localref {}", Trc::local_refcount(&trc));
    println!("atomicref {}", Trc::atomic_count(&trc));
    drop(trc);
    println!("localref {}", Trc::local_refcount(&alt));
    println!("atomicref {}", Trc::atomic_count(&alt));
    println!();
}

#[test]
fn test_multithread1() {
    let data = Data {
        string: String::from("This is data."),
        int: 123,
    };

    let thread_trc_main = Trc::new(data);
    println!(
        "Local reference count in thread0: {}",
        Trc::local_refcount(&thread_trc_main)
    );
    let shared = SharedTrc::from_trc(&thread_trc_main);
    let handle = thread::spawn(move || {
        let mut trc = SharedTrc::to_trc(shared);
        println!("Thread1 Deref test! {}", trc.int);
        println!("DerefMut test");
        (*unsafe { Trc::deref_mut(&mut trc) }).string = String::from("This is the new data");
    });
    handle.join().unwrap();
    println!(
        "Atomic reference count after thread1: {}",
        Trc::atomic_count(&thread_trc_main)
    );
    println!("Thread0 Deref test! {}", thread_trc_main.string);
}

#[test]
fn test_multithread2() {
    let trc = Trc::new(100);
    let shared = SharedTrc::from_trc(&trc);
    let handle = thread::spawn(move || {
        let mut trc = SharedTrc::to_trc(shared);
        println!("{:?}", *trc);
        *unsafe { Trc::deref_mut(&mut trc) } = 200;
    });
    handle.join().unwrap();
    println!("{}", *trc);
    assert_eq!(*trc, 200);
}

#[test]
fn test_weak() {
    let trc = Trc::new(100);
    let weak = Weak::from_trc(&trc);
    let mut new_trc = Weak::to_trc(&weak).unwrap();
    println!("Deref test! {}", *new_trc);
    println!("DerefMut test");
    *unsafe { Trc::deref_mut(&mut new_trc) } = 200;
    println!("Deref test! {}", *new_trc);
}

#[test]
fn test_multithread_weak() {
    let trc = Trc::new(100);
    let weak = Weak::from_trc(&trc);
    let handle = thread::spawn(move || {
        let mut trc = Weak::to_trc(&weak).unwrap();
        println!("{:?}", *trc);
        *unsafe { Trc::deref_mut(&mut trc) } = 200;
        println!("Atomic: {}", Trc::atomic_count(&trc));
    });
    handle.join().unwrap();
    println!("{}", *trc);
    assert_eq!(*trc, 200);
    println!("Atomic: {}", Trc::atomic_count(&trc));
}

#[test]
fn test_dyn() {
    trait Vehicle {
        fn drive(&self);
    }

    struct Truck;

    impl Vehicle for Truck {
        fn drive(&self) {
            println!("Truck is driving");
        }
    }

    let vehicle = Trc::new(Box::new(Truck));
    vehicle.drive();
}

#[test]
fn test_weak_drop() {
    let trc = Trc::new(100);
    let weak = Weak::from_trc(&trc);
    println!("atomic {}", Trc::atomic_count(&trc));
    drop(trc);
    assert!(Weak::to_trc(&weak).is_none())
}
