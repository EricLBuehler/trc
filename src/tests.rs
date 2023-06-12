use std::thread;

use crate::trc::Trc;

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
    trc.string = String::from("This is also data");
    println!("Deref test! {}", trc.string);
}

#[test]
fn test_multithread1() {
    let data = Data {
        string: String::from("This is data."),
        int: 123,
    };

    let thread_trc_main = Trc::new(data);
    let mut thread_trc_thread = thread_trc_main.clone_across_thread();
    let handle = thread::spawn(move || {
        println!("Thread1 Deref test! {}", thread_trc_thread.int);
        println!("DerefMut test");
        thread_trc_thread.string = String::from("This is the new data");
        println!(
            "Atomic reference count in thread: {}",
            Trc::atomic_count(&thread_trc_thread)
        );
    });
    handle.join().unwrap();
    println!(
        "Atomic reference count after thread: {}",
        Trc::atomic_count(&thread_trc_main)
    );
    println!("Thread0 Deref test! {}", thread_trc_main.string);
}

#[test]
fn test_multithread2() {
    let trc = Trc::new(100);
    let mut trc2 = trc.clone_across_thread();

    let handle = thread::spawn(move || {
        println!("{:?}", *trc2);
        *trc2 = 200;
    });
    handle.join().unwrap();
    println!("{}", *trc);
    assert_eq!(*trc, 200);
}
