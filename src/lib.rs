//! These docs are a work in progress. If you find any issues, I would greatly appreciate you submitting a pull request or issue to the [GitHub page](https://github.com/EricLBuehler/trc).
//! `Trc<T>` is a heap-allocated smart pointer for sharing data across threads is a thread-safe manner without putting locks on the data.
//! `Trc<T>` stands for: Thread Reference Counted.
//! `Trc<T>` provides a shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
//! It implements biased reference counting, which is based on the observation that most objects are only used by one thread.
//! This means that two reference counts can be created: one for thread-local use, and one atomic one for sharing between threads.
//! This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.
//! The type parameter for `Trc<T>`, `T`, is `?Sized`. This allows `Trc<T>` to be used as a wrapper over trait objects, as `Trc<T>` itself is sized.

pub mod trc;
pub use crate::trc::Trc;
pub use crate::trc::Weak;

#[cfg(test)]
mod tests;
