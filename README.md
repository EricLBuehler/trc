# Trc
Trc is a biased reference-counted smart pointer for Rust that allows for interior mutability.
`Trc<T>` is a heap-allocated smart pointer for sharing data across threads is a thread-safe manner without putting locks on the data.
`Trc<T>` stands for: Thread Reference Counted.
`Trc<T>` provides a shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
It implements biased reference counting, which is based on the observation that most objects are only used by one thread.
This means that two reference counts can be created: one for thread-local use, and one atomic one for sharing between threads.
This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.
The type parameter for `Trc<T>`, `T`, is `?Sized`. This allows `Trc<T>` to be used as a wrapper over trait objects, as `Trc<T>` itself is sized.

## Clone behavior
When a `Trc<T>` is cloned, it's internal (wrapped) data stays at the same memory location, but a new `Trc<T>` is constructed and returned.
This makes a `clone` a relatively inexpensive operation because only a wrapper is constructed.
This new `Trc<T>` points to the same memory, and all `Trc<T>`s that point to that memory in that thread will have their thread-local reference counts incremented
and their atomic reference counts unchanged.

For use of threads, `Trc<T>` has a `clone_across_thread` method. This is relatively expensive; it allocates memory on the heap. However, calling the method
is most likely something that will not be done in loop.
`clone_across_thread` increments the atomic reference count - that is, the reference count that tells how many threads are using the object.

## Drop behavior

When a `Trc<T>` is dropped the thread-local reference count is decremented. If it is zero, the atomic reference count is also decremented.
If the atomic reference count is zero, then the internal data is dropped. Regardless of wherether the atomic refernce count is zero, the
local `Trc<T>` is dropped.

## `Deref` and `DerefMut` behavior
For ease of developer use, `Trc<T>` comes with `Deref` and `DerefMut` implemented to allow internal mutation.
`Trc<T>` automatically dereferences to `&T` or `&mut T`. This allows method calls and member acess of `T`.
To prevent name clashes, `Trc<T>`'s functions are associated.

# Examples

Example in a single thread:
```rust
use trc::Trc;

let mut trc = Trc::new(100);
assert_eq!(*trc, 100);
*trc = 200;
assert_eq!(*trc, 200);
```

Example with multiple threads:
```rust
use std::thread;
use trc::Trc;

let trc = Trc::new(100);
let mut trc2 = Trc::clone_across_thread(&trc);

let handle = thread::spawn(move || {
    *trc2 = 200;
});

handle.join().unwrap();
assert_eq!(*trc, 200);
```
