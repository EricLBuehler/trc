//! These docs are a work in progress. If you find any issues, I would greatly appreciate you submitting a pull request or issue to the [GitHub page](https://github.com/EricLBuehler/trc).
//!
//! `Trc<T>` is a heap-allocated smart pointer for sharing data across threads is a thread-safe manner without putting locks on the data.
//! `Trc<T>` stands for: Thread Reference Counted.
//! `Trc<T>` provides a shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
//! It implements biased reference counting, which is based on the observation that most objects are only used by one thread.
//! This means that two reference counts can be created: one for thread-local use, and one atomic one for sharing between threads.
//! This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.
//!
//! A cycle between `Trc` pointers cannot be deallocated as the reference counts will never reach zero. The solution is a `Weak<T>`.
//! A `Weak<T>` is a non-owning reference to the data held by a `Trc<T>`.
//! They break reference cycles by adding a layer of indirection and act as an observer. They cannot even access the data directly, and
//! must be converted back into `Trc<T>`. `Weak<T>` does not keep the value alive (whcih can be dropped), and only keeps the backing allocation alive.

pub mod trc;
pub use crate::trc::Trc;
pub use crate::trc::Weak;

#[cfg(test)]
mod tests;
