# Trc
![rustc 1.70.0 stable](https://img.shields.io/badge/rustc-1.70.0-brightgreen)
[![MIT License](https://img.shields.io/badge/License-MIT-informational)](LICENSE)
![Build status](https://github.com/EricLBuehler/trc/actions/workflows/build.yml/badge.svg)
![Docs status](https://github.com/EricLBuehler/trc/actions/workflows/docs.yml/badge.svg)
![Tests status](https://github.com/EricLBuehler/trc/actions/workflows/tests.yml/badge.svg)

`Trc` is a performant heap-allocated smart pointer for Rust that implements a version of biased reference counting.
`Trc<T>` stands for: Thread Reference Counted.
`Trc<T>` provides a shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
It implements a custom version of biased reference counting, which is based on the observation that most objects are only used by one thread.
This means that two reference counts can be created: one for thread-local use, and one atomic one for sharing between threads.
This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.

A cycle between `Trc` pointers cannot be deallocated as the reference counts will never reach zero. The solution is a `Weak<T>`.
A `Weak<T>` is a non-owning reference to the data held by a `Trc<T>`.
They break reference cycles by adding a layer of indirection and act as an observer. They cannot even access the data directly, and
must be converted back into `Trc<T>`. `Weak<T>` does not keep the value alive (whcih can be dropped), and only keeps the backing allocation alive.

To soundly implement thread safety `Trc<T>` does not itself implement `Send` or `Sync`. However, `SharedTrc<T>` does, and it is the only way to safely send a `Trc<T>` across threads. See `SharedTrc` for it's API, which is similar to that of `Weak`.


## Examples

Example of `Trc<T>` in a single thread:
```rust
use trc::Trc;

let mut trc = Trc::new(100);
assert_eq!(*trc, 100);
*unsafe { Trc::get_mut(&mut trc) }.unwrap() = 200;
assert_eq!(*trc, 200);
```

Example of `Trc<T>` with multiple threads:
```rust
use std::thread;
use trc::Trc;
use trc::SharedTrc;

let trc = Trc::new(100);
let shared = SharedTrc::from_trc(&trc);
let handle = thread::spawn(move || {
    let trc = SharedTrc::to_trc(shared);
    assert_eq!(*trc, 100);
});

handle.join().unwrap();
assert_eq!(*trc, 100);
```

Example of `Weak<T>` in a single thread:
```rust
use trc::Trc;
use trc::Weak;

let trc = Trc::new(100);
let weak = Trc::downgrade(&trc);
let mut new_trc = Weak::upgrade(&weak).unwrap();
assert_eq!(*new_trc, 100);
drop(trc);
drop(weak);
*unsafe { Trc::get_mut(&mut new_trc) }.unwrap() = 200;
assert_eq!(*new_trc, 200);
```

Example of `Weak<T>` with multiple threads:
```rust
use std::thread;
use trc::Trc;
use trc::Weak;

let trc = Trc::new(100);
let weak = Trc::downgrade(&trc);

let handle = thread::spawn(move || {
    let trc = Weak::upgrade(&weak).unwrap();
    assert_eq!(*trc, 100);
});
handle.join().unwrap();
assert_eq!(*trc, 100);
```

## Benchmarks
Benchmarks via Criterion. As can be seen, `Trc`'s performance realy shines when there are many Clones.
The reason `Trc` does not do as well for fewer operations is because it needs to allocate `n+1` blocks of memory for `n` threads, and
so for 1 thread, there are 2 allocations. However, after the initial allocations, `Trc` performs very well - 3.94x `Arc`'s time for Clones. 

### Clone
| Type | Mean time |
| --- | ----------- |
| Trc | 35.926ms |
| Arc | 37.032ns |
| Rc | 15.866ns |

### Multiple Clone (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 337.210ns |
| Arc | 1327.000ns |
| Rc | 293.71ns |

### Deref
| Type | Mean time |
| --- | ----------- |
| Trc | 23.613ns |
| Arc | 23.735ns |
| Rc | 12.462ns |

### Multiple Deref (100 times)
| Type | Mean time |
| --- | ----------- |
| Trc | 51.166ns |
| Arc | 55.585ns |
| Rc | 41.808ns |

## Use
To use `Trc`, simply run `cargo add trc`, or add `trc = "1.1.18"`. Optionally, you can always use the latest version by adding `trc = {git = "https://github.com/EricLBuehler/trc.git"}`.