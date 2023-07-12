//! If you find any issues with `Trc`, I would greatly appreciate you submitting a pull request or issue to the [GitHub page](https://github.com/EricLBuehler/trc).
//!
//! `Trc` is a performant heap-allocated smart pointer for Rust that implements a thread reference counting.
//! `Trc` stands for: Thread Reference Counted.
//! [`Trc`] provides shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
//! It implements thread reference counting, which is based on the observation that most objects are only used by one thread.
//! This means that two reference counts can be created: one for local thread use, and one atomic one for sharing between threads.
//! Thread reference counting sets the atomic reference count to the number of threads using the data.
//!
//! ## Breaking reference cycles with `Weak<T>`
//! A cycle between `Trc` pointers cannot be deallocated as the reference counts will never reach zero. The solution is a `Weak<T>`.
//! A `Weak<T>` is a non-owning reference to the data held by a `Trc<T>`.
//! They break reference cycles by adding a layer of indirection and act as an observer. They cannot even access the data directly, and
//! must be converted back into `Trc<T>`. `Weak<T>` does not keep the value alive (whcih can be dropped), and only keeps the backing allocation alive.
//! See [`Weak`] for more information.
//!
//! ## Sending data accross threads with `SharedTrc<T>`
//! To soundly implement thread safety `Trc<T>` does not itself implement [`Send`] or [`Sync`].
//! However, `SharedTrc<T>` does, and it is the only way to safely send a `Trc<T>` across
//! threads. See [`SharedTrc`] for it's API, which is similar to that of `Weak`.
//! See [`SharedTrc`] for it's API, which is similar to that of [`Weak`].

extern crate alloc;

pub mod trc;
pub use crate::trc::SharedTrc;
pub use crate::trc::Trc;
pub use crate::trc::Weak;

#[cfg(test)]
mod tests;
