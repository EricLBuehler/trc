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
    Trc::get_mut(&mut trc).unwrap().string = String::from("This is also data");
    println!("Deref test! {}", trc.string);
}

#[test]
fn test_singlethreaded2() {
    let mut trc = Trc::new(100);
    assert_eq!(*trc, 100);
    *Trc::get_mut(&mut trc).unwrap() = 200;
    assert_eq!(*trc, 200);
}

#[test]
fn test_refcount() {
    let trc = Trc::new(100);
    let alt = trc.clone();
    println!();
    println!("localref {}", Trc::local_count(&trc));
    println!("atomicref {}", Trc::atomic_count(&trc));
    let _shared = SharedTrc::from_trc(&trc);
    println!("localref {}", Trc::local_count(&trc));
    println!("atomicref {}", Trc::atomic_count(&trc));
    drop(trc);
    println!("localref {}", Trc::local_count(&alt));
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
        Trc::local_count(&thread_trc_main)
    );
    let shared = SharedTrc::from_trc(&thread_trc_main);
    let handle = thread::spawn(move || {
        let trc = SharedTrc::to_trc(shared);
        println!("Thread1 Deref test! {}", trc.int);
        println!("DerefMut test");
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
        let trc = SharedTrc::to_trc(shared);
        println!("{:?}", *trc);
    });
    handle.join().unwrap();
    println!("{}", *trc);
    assert_eq!(*trc, 100);
}

#[test]
fn test_weak() {
    let trc = Trc::new(100);
    let weak = Trc::downgrade(&trc);
    let mut new_trc = Weak::upgrade(&weak).unwrap();
    println!("Deref test! {}", *new_trc);
    println!("DerefMut test");
    drop(weak);
    drop(trc);
    *Trc::get_mut(&mut new_trc).unwrap() = 200;
    println!("Deref test! {}", *new_trc);
}

#[test]
fn test_multithread_weak() {
    let trc = Trc::new(100);
    let weak = Trc::downgrade(&trc);
    let handle = thread::spawn(move || {
        let trc = Weak::upgrade(&weak).unwrap();
        println!("{:?}", *trc);
        println!("Atomic: {}", Trc::atomic_count(&trc));
    });
    handle.join().unwrap();
    println!("{}", *trc);
    assert_eq!(*trc, 100);
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
    let weak = Trc::downgrade(&trc);
    println!("atomic {}", Trc::atomic_count(&trc));
    println!("weak {}", Trc::weak_count(&trc));
    drop(trc);
    println!("DROPPED");
    assert!(Weak::upgrade(&weak).is_none())
}

#[test]
fn test_from_slice() {
    let vec = (1..100).collect::<Vec<i32>>();
    let slice = &vec[20..50];
    let trc = Trc::<[i32]>::from(slice);
    assert_eq!(&*trc, slice);
}

#[test]
fn readme_single_trc() {
    let mut trc = Trc::new(100);
    assert_eq!(*trc, 100);
    *Trc::get_mut(&mut trc).unwrap() = 200;
    assert_eq!(*trc, 200);
}

#[test]
fn readme_multi_trc() {
    let trc = Trc::new(100);
    let shared = SharedTrc::from_trc(&trc);
    let handle = thread::spawn(move || {
        let trc = SharedTrc::to_trc(shared);
        assert_eq!(*trc, 100);
    });

    handle.join().unwrap();
    assert_eq!(*trc, 100);
}

#[test]
fn readme_single_weak() {
    let trc = Trc::new(100);
    let weak = Trc::downgrade(&trc);
    let mut new_trc = Weak::upgrade(&weak).unwrap();
    assert_eq!(*new_trc, 100);
    drop(trc);
    drop(weak);
    *Trc::get_mut(&mut new_trc).unwrap() = 200;
    assert_eq!(*new_trc, 200);
}

#[test]
fn readme_multi_weak() {
    let trc = Trc::new(100);
    let weak = Trc::downgrade(&trc);

    let handle = thread::spawn(move || {
        let trc = Weak::upgrade(&weak).unwrap();
        assert_eq!(*trc, 100);
    });
    handle.join().unwrap();
    assert_eq!(*trc, 100);
}
