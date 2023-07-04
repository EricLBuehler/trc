# Trc
![rustc 1.70.0 stable](https://img.shields.io/badge/rustc-1.70.0-brightgreen)
[![MIT License](https://img.shields.io/badge/License-MIT-informational)](LICENSE)
![Build status](https://github.com/EricLBuehler/trc/actions/workflows/build.yml/badge.svg)
![Docs status](https://github.com/EricLBuehler/trc/actions/workflows/docs.yml/badge.svg)
![Tests status](https://github.com/EricLBuehler/trc/actions/workflows/tests.yml/badge.svg)

`Trc` is a performant biased reference-counted smart pointer for Rust.
It is a heap-allocated smart pointer for sharing data across threads is a thread-safe manner without putting locks on the data.
`Trc<T>` stands for: Thread Reference Counted.
`Trc<T>` provides a shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
It implements a custom version of biased reference counting, which is based on the observation that most objects are only used by one thread.
This means that two reference counts can be created: one for thread-local use, and one atomic one for sharing between threads.
This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.

A cycle between `Trc` pointers cannot be deallocated as the reference counts will never reach zero. The solution is a `Weak<T>`.
A `Weak<T>` is a non-owning reference to the data held by a `Trc<T>`.
They break reference cycles by adding a layer of indirection and act as an observer. They cannot even access the data directly, and
must be converted back into `Trc<T>`. `Weak<T>` does not keep the value alive (whcih can be dropped), and only keeps the backing allocation alive.

To soundly implement thread safety `Trc<T>` does not itself implement [`Send`] or [`Sync`]. However, `SharedTrc<T>` does, and it is the only way to safely send a `Trc<T>` across threads. See `SharedTrc` for it's API, which is similar to that of `Weak`.

`Trc` will automatically compile to use either locks or atomics, depending on the system. By default, `Trc` uses `std`.
However, `Trc` can be compiled without `std`. When compiling withput `std`, locks and atomics are still available, and will be automatically compiled
depending on the system. This is enabled using the `nostd` feature. Compilation with locks or atomics can be forced with a feature flag.

## Examples

Example of `Trc<T>` in a single thread:
```rust
use trc::Trc;

let mut trc = Trc::new(100);
assert_eq!(*trc, 100);
*trc = 200;
assert_eq!(*trc, 200);
```

Example of `Trc<T>` with multiple threads:
```rust
use std::thread;
use trc::Trc;
use trc::SharedTrc;

let trc = Trc::new(100);
let shared = SharedTrc::from_trc(&thread_trc_main);
let handle = thread::spawn(move || {
    let mut trc = SharedTrc::to_trc(shared);
    *trc2 = 200;
});

handle.join().unwrap();
assert_eq!(*trc, 200);
```

Example of `Weak<T>` in a single thread:
```rust
use trc::Trc;
use trc::Weak;

let trc = Trc::new(100);
let weak = Weak::from_trc(&trc);
let mut new_trc = Weak::to_trc(&weak).unwrap();
println!("Deref test! {}", *new_trc);
println!("DerefMut test");
*new_trc = 200;
println!("Deref test! {}", *new_trc);
```

Example of `Weak<T>` with multiple threads:
```rust
use std::thread;
use trc::Trc;
use trc::Weak;

let trc = Trc::new(100);
let weak = Weak::from_trc(&trc);

let handle = thread::spawn(move || {
    let mut trc = Weak::to_trc(&weak).unwrap();
    println!("{:?}", *trc);
    *trc = 200;
});
handle.join().unwrap();
println!("{}", *trc);
assert_eq!(*trc, 200);
```

## Benchmarks
Benchmarks are conducted by Criterion.
### Clone
| Type | Mean time |
| --- | ----------- |
| Trc | 40.462ns |
| Arc | 24.998ns |
| Rc | 12.354ns |

### Deref
| Type | Mean time |
| --- | ----------- |
| Trc | 28.549ns |
| Arc | 26.859ns |
| Rc | 12.856ns |