# Trc
Trc is a biased reference-counted smart pointer for Rust that allows for interior mutability.
`Trc` is a smart pointer for sharing data across threads is a thread-safe manner without putting locks on the data.
`Trc` stands for: Thread Reference Counted
It implements biased reference counting, which is based on the observation that most objects are only used by one thread.
This means that two refernce counts can be created: one for thread-local use, and one atomic one (with a lock) for sharing between threads.
This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.

When a `Trc` is dropped, then the thread-local reference count is decremented. If it is zero, the atomic reference count is also decremented.
If the atomic reference count is zero, then the internal data is dropped. Regardless of wherether the atomic refernce count is zero, the
local `Trc` is dropped.

For ease of developer use, `Trc` comes with `Deref` and `DerefMut` implemented to allow internal mutation.

Example in a single thread:
```rust
use trc::Trc;
let mut trc = Trc::new(100);
println!("{}", *trc);
*trc = 200;
println!("{}", *trc);
```

Example with multiple threads:
```rust
use std::thread;
use trc::Trc;

let trc = Trc::new(100);
let mut trc2 = trc.clone_across_thread();

let handle = thread::spawn(move || {
    println!("{}", *trc2);
    *trc2 = 200;
});

handle.join().unwrap();
println!("{}", *trc);
assert_eq!(*trc, 200);
```

See https://docs.rs/trc/latest/trc/ for the latest docs.