//! If you find any issues with `Trc`, I would greatly appreciate you submitting a pull request or issue to the [GitHub page](https://github.com/EricLBuehler/trc).
//!
//! `Trc<T>` is a heap-allocated smart pointer for sharing data across threads is a thread-safe and performant manner.
//! `Trc<T>` stands for: Thread Reference Counted.
//! `Trc<T>` provides shared ownership of the data similar to `Arc<T>` and `Rc<T>`. In addition, it also provides interior mutability.
//! It implements a custom version of biased reference counting, which is based on the observation that most objects are only used by one thread.
//! This means that two reference counts can be created: one for thread-local use, and one atomic one for sharing between threads.
//! This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.
//!
//! A cycle between `Trc` pointers cannot be deallocated as the reference counts will never reach zero. The solution is a `Weak<T>`.
//! A `Weak<T>` is a non-owning reference to the data held by a `Trc<T>`.
//! They break reference cycles by adding a layer of indirection and act as an observer. They cannot even access the data directly, and
//! must be converted back into `Trc<T>`. `Weak<T>` does not keep the value alive (which can be dropped), and only keeps the backing allocation alive.

//#![cfg_attr(not(test), no_std)]

extern crate alloc;
#[cfg(feature = "force_lock")]
extern crate std;

pub mod trc;
pub use crate::trc::SharedTrc;
pub use crate::trc::Trc;
pub use crate::trc::Weak;

#[cfg(test)]
mod tests;
