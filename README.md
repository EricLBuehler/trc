# Trc
[![MIT License](https://img.shields.io/badge/License-MIT-informational)](LICENSE)
![Build status](https://github.com/EricLBuehler/trc/actions/workflows/build.yml/badge.svg)
![Docs status](https://github.com/EricLBuehler/trc/actions/workflows/docs.yml/badge.svg)
![Tests status](https://github.com/EricLBuehler/trc/actions/workflows/tests.yml/badge.svg)

`Trc` is a performant heap-allocated smart pointer for Rust that implements thread reference counting.
`Trc` stands for: Thread Reference Counted.
`Trc` provides a shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
It implements thread reference counting, which is based on the observation that most objects are only used by one thread.
This means that two reference counts can be created: one for thread-local use, and one atomic one for sharing between threads.
Thread reference counting sets the atomic reference count to the number of threads using the data.

A cycle between `Trc` pointers cannot be deallocated as the reference counts will never reach zero. The solution is a `Weak<T>`.
A `Weak<T>` is a non-owning reference to the data held by a `Trc<T>`.
They break reference cycles by adding a layer of indirection and act as an observer. They cannot even access the data directly, and
must be converted back into `Trc<T>`. `Weak<T>` does not keep the value alive (which can be dropped), and only keeps the backing allocation alive.

To soundly implement thread safety `Trc<T>` does not itself implement `Send` or `Sync`. However, `SharedTrc<T>` does, and it is the only way to 
safely send a `Trc<T>` across threads. See `SharedTrc` for it's API, which is similar to that of `Weak`.

Because `Trc` is not part of the standard library, the `CoerceUnsized` and `Receiver` traits cannot currently be implemented by default. However, `Trc` provides `dyn_unstable` trait which enables the above traits for `Trc` and `SharedTrc` and must be used with nightly Rust (`cargo +nightly ...`).

## Examples
See examples [here](EXAMPLES.md).

## Benchmarks 

Click [here](BENCHMARKS.md) for more benchmarks. Multiple different operating systems, CPUs, and architectures are tested. 

![Trc vs Arc performance](./figures/performance.png)

## Use
To use `Trc`, simply run `cargo add trc`, or add `trc = "1.2.2"`. Optionally, you can always use the latest version by adding `trc = {git = "https://github.com/EricLBuehler/trc.git"}`.
