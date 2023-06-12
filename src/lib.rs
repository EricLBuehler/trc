//! These docs are a work in progress. If you find any issues, I would greatly appreciate you submitting a pull request or issue to the [GitHub page](https://github.com/EricLBuehler/trc)
//! `Trc` is a smart pointer for sharing data across threads is a thread-safe manner without putting locks on the data.
//! `Trc` stands for: Thread Reference Counted
//! It implements biased reference counting, which is based on the observation that most objects are only used by one thread.
//! This means that two refernce counts can be created: one for thread-local use, and one atomic one (with a lock) for sharing between threads.
//! This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.
//!

pub mod trc;
pub use crate::trc::Trc;

#[cfg(test)]
mod tests;
