#![allow(dead_code)]
#![allow(clippy::mut_from_ref)]

use std::{
    borrow::{Borrow, BorrowMut},
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

#[cfg(not(target_has_atomic = "ptr"))]
use std::sync::RwLock;

#[cfg(target_has_atomic = "ptr")]
use std::sync::atomic::AtomicUsize;

pub struct AtomicThreadTrc<T> {
    #[cfg(not(target_has_atomic = "ptr"))]
    atomicref: RwLock<usize>,
    #[cfg(target_has_atomic = "ptr")]
    atomicref: AtomicUsize,
    pub data: T,
}

struct LocalThreadTrc<T> {
    atomicref: NonNull<AtomicThreadTrc<T>>,
    threadref: usize,
}

/// `Trc` is a smart pointer for sharing data across threads is a thread-safe manner without putting locks on the data.
/// `Trc` stands for: Thread Reference Counted
/// It impkements biased reference counting, which is based on the observation that most objects are only used by one thread.
/// This means that two refernce counts can be created: one for thread-local use, and one atomic one (with a lock) for sharing.
/// This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.
/// 
/// When a `Trc` is dropped, then the thread-local reference count is decremented. If it is zero, the atomic reference count is also decremented.
/// If the atomic reference count is zero, then the internal data is dropped. Regardless of wherether the atomic refernce count is zero, the
/// local `Trc` is dropped.
///
/// For ease of developer use, `Trc` comes with [`Deref`] and [`DerefMut`] implemented to allow internal mutation.
///
/// Example in a single thread:
/// ```
/// let trc = Trc::new(100);
/// println!("{}", trc);
/// *trc = 200;
/// println!("{}", trc);
/// ```
///
/// Example with multiple threads:
/// ```
/// use std::thread;
///
/// let trc = Trc::new(100);
/// let mut trc2 = trc.clone_across_thread();
///
/// let handle = thread::spawn(move || {
///     println!("{}", *trc2);
///     *trc2 = 200;
/// });
///
/// handle.join().unwrap();
/// println!("{}", *trc);
/// assert_eq!(*trc, 200);
/// ```
///
#[derive(PartialEq, Eq)]
pub struct Trc<T> {
    data: NonNull<LocalThreadTrc<T>>,
}

impl<T> Trc<T> {
    /// Creates a new `Trc` from the provided data.
    /// ```
    /// let trc = Trc::new(100);
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    pub fn new(value: T) -> Self {
        let atomicthreadata = AtomicThreadTrc {
            atomicref: AtomicUsize::new(0),
            data: value,
        };

        let abx = Box::new(atomicthreadata);

        let localthreadtrc = LocalThreadTrc {
            atomicref: NonNull::from(Box::leak(abx)),
            threadref: 1,
        };

        let tbx = Box::new(localthreadtrc);

        Trc {
            data: NonNull::from(Box::leak(tbx)),
        }
    }

    /// Creates a new `Trc` from the provided data.
    /// ```
    /// let trc = Trc::new(100);
    /// ```
    #[inline]
    #[cfg(not(target_has_atomic = "ptr"))]
    pub fn new(value: T) -> Self {
        let atomicthreadata = AtomicThreadTrc {
            atomicref: RwLock::new(0),
            data: value,
        };

        let abx = Box::new(atomicthreadata);

        let localthreadtrc = LocalThreadTrc {
            atomicref: NonNull::from(Box::leak(abx)),
            threadref: 1,
        };

        let tbx = Box::new(localthreadtrc);

        Trc {
            data: NonNull::from(Box::leak(tbx)),
        }
    }

    /// Return the local thread count of the object. This is how many `Trc`s are using the data referenced by this `Trc`.
    /// ```
    /// let trc = Trc::new(100);
    /// assert!(Trc::thread_count(trc) == 1)
    /// ```
    #[inline]
    pub fn thread_count(this: &Self) -> usize {
        return this.inner().threadref;
    }

    /// Return the atomic reference count of the object. This is how many threads are using the data referenced by this `Trc`./// ```
    /// use std::thread;
    ///
    /// let trc = Trc::new(100);
    /// let mut trc2 = trc.clone_across_thread();
    ///
    /// let handle = thread::spawn(move || {
    ///     println!("{}", *trc2);
    ///     *trc2 = 200;
    ///     assert_eq!(Trc::atomic_count(&trc), 2);
    /// });
    ///
    /// handle.join().unwrap();
    /// assert_eq!(Trc::atomic_count(&trc), 1);
    /// println!("{}", *trc);
    /// assert_eq!(*trc, 200);
    /// ```
    #[inline]
    #[cfg(not(target_has_atomic = "ptr"))]
    pub fn atomic_count(this: &Self) -> usize {
        let mut readlock = this.inner_atomic().atomicref.try_write();

        while readlock.is_err() {
            readlock = this.inner_atomic().atomicref.try_write();
        }
        *readlock.unwrap()
    }

    /// Return the atomic reference count of the object. This is how many threads are using the data referenced by this `Trc`.
    /// ```
    /// use std::thread;
    ///
    /// let trc = Trc::new(100);
    /// let mut trc2 = trc.clone_across_thread();
    ///
    /// let handle = thread::spawn(move || {
    ///     println!("{}", *trc2);
    ///     *trc2 = 200;
    ///     assert_eq!(Trc::atomic_count(&trc), 2);
    /// });
    ///
    /// handle.join().unwrap();
    /// assert_eq!(Trc::atomic_count(&trc), 1);
    /// println!("{}", *trc);
    /// assert_eq!(*trc, 200);
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    pub fn atomic_count(this: &Self) -> usize {
        this.inner_atomic()
            .atomicref
            .load(std::sync::atomic::Ordering::Acquire)
    }

    #[inline]
    fn inner(&self) -> &LocalThreadTrc<T> {
        return unsafe { self.data.as_ref() };
    }

    #[inline]
    fn inner_atomic(&self) -> &AtomicThreadTrc<T> {
        return unsafe { self.data.as_ref().atomicref.as_ref() };
    }

    #[inline]
    fn inner_mut(&self) -> &mut LocalThreadTrc<T> {
        unsafe { &mut *self.data.as_ptr() }
    }

    #[inline]
    fn inner_atomic_mut(&self) -> &mut AtomicThreadTrc<T> {
        unsafe { &mut *(*self.data.as_ptr()).atomicref.as_ptr() }
    }

    /// Clone a `Trc` across threads. This is necessary because otherwise the atomic reference count will not be incremented.
    /// ```
    /// let trc = Trc::new(100);
    /// let trc2 = trc.clone_across_thread();
    /// ```
    #[inline]
    #[cfg(not(target_has_atomic = "ptr"))]
    pub fn clone_across_thread(&self) -> Self {
        let mut writelock = self.inner_atomic().atomicref.try_write();

        while writelock.is_err() {
            writelock = self.inner_atomic().atomicref.try_write();
        }
        let mut writedata = writelock.unwrap();

        *writedata += 1;

        let localthreadtrc = LocalThreadTrc {
            atomicref: self.inner().atomicref,
            threadref: 1,
        };

        let tbx = Box::new(localthreadtrc);

        return Trc {
            data: NonNull::from(Box::leak(tbx)),
        };
    }

    /// Clone a `Trc` across threads (increase it's atomic reference count). This is necessary because otherwise the atomic reference count will not be incremented.
    /// ```
    /// let trc = Trc::new(100);
    /// let trc2 = trc.clone_across_thread();
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    pub fn clone_across_thread(&self) -> Self {
        self.inner_atomic()
            .atomicref
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);

        let localthreadtrc = LocalThreadTrc {
            atomicref: self.inner().atomicref,
            threadref: 1,
        };

        let tbx = Box::new(localthreadtrc);

        return Trc {
            data: NonNull::from(Box::leak(tbx)),
        };
    }

    /// Checks if the other `Trc` is equal to this one according to their internal pointers.
    /// ```
    /// let trc1 = Trc::new(100);
    /// let trc2 = trc1.clone();
    /// assert!(Trc::ptr_eq(&trc1, &trc2));
    /// ```
    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        return this.inner().atomicref.as_ptr() == other.inner().atomicref.as_ptr();
    }

    /// Gets the raw pointer to the most inner layer of `Trc`.
    /// The `AtomicThreadTrc` type only contains the data as its only public member.
    /// ```
    /// let trc = Trc::new(100);
    /// println!("{}", Trc::as_ptr(&trc) as usize)
    /// ```
    #[inline]
    pub fn as_ptr(this: &Self) -> *mut AtomicThreadTrc<T> {
        return this.inner().atomicref.as_ptr();
    }
}

impl<T> Deref for Trc<T> {
    type Target = T;

    /// Get an immutable reference to the internal data.
    /// ```
    /// let mut trc = Trc::new(100);
    /// println!("{}", trc);
    /// let refr = trc.deref();
    /// println!("{}", refr);
    /// ```
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner_atomic().borrow().data
    }
}

impl<T> DerefMut for Trc<T> {
    /// Get a &mut reference to the internal data.
    /// ```
    /// let mut trc = Trc::new(100);
    /// *trc = 200;
    /// let mutref = trc.deref_mut();
    /// *mutref = 300;
    /// ```
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner_atomic_mut().borrow_mut().data
    }
}

impl<T> Drop for Trc<T> {
    #[inline]
    #[cfg(not(target_has_atomic = "ptr"))]
    fn drop(&mut self) {
        self.inner_mut().threadref -= 1;
        if self.inner().threadref == 0 {
            let mut writelock = self.inner_atomic().atomicref.try_write();

            while writelock.is_err() {
                writelock = self.inner_atomic().atomicref.try_write();
            }
            let mut writedata = writelock.unwrap();

            *writedata -= 1;

            if *writedata == 0 {
                std::mem::drop(writedata);

                unsafe { Box::from_raw(self.inner().atomicref.as_ptr()) };
                unsafe { Box::from_raw(self.data.as_ptr()) };
            }
        }
    }

    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    fn drop(&mut self) {
        self.inner_mut().threadref -= 1;
        if self.inner().threadref == 0 {
            let res = self
                .inner_atomic()
                .atomicref
                .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);

            if res == 0 {
                unsafe { Box::from_raw(self.inner().atomicref.as_ptr()) };
                unsafe { Box::from_raw(self.data.as_ptr()) };
            }
        }
    }
}

impl<T> Clone for Trc<T> {
    /// Clone a `Trc` (increase it's local reference count). This can only be used to clone an object that will only stay in one thread.
    /// ```
    /// let trc = Trc::new(100);
    /// let trc2 = trc.clone();
    /// ```
    #[inline]
    fn clone(&self) -> Self {
        self.inner_mut().threadref += 1;

        Trc { data: self.data }
    }
}

unsafe impl<T> Send for Trc<T> {}
unsafe impl<T> Sync for Trc<T> {}
