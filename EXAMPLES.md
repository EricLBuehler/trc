
## Examples

Example of `Trc<T>` in a single thread:
```rust
use trc::Trc;

let mut trc = Trc::new(100);
assert_eq!(*trc, 100);
*Trc::get_mut(&mut trc).unwrap() = 200;
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
let mut new_trc = weak.upgrade().unwrap();
assert_eq!(*new_trc, 100);
drop(trc);
drop(weak);
*Trc::get_mut(&mut new_trc).unwrap() = 200;
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
    let trc = weak.upgrade().unwrap();
    assert_eq!(*trc, 100);
});
handle.join().unwrap();
assert_eq!(*trc, 100);
```