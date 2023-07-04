#![allow(dead_code)]
#![allow(clippy::mut_from_ref)]

use core::{
    borrow::Borrow,
    fmt::{Debug, Display, Pointer},
    hash::{Hash, Hasher},
    ops::Deref,
    pin::Pin,
    ptr::NonNull,
};

#[cfg(any(
    all(not(target_has_atomic = "ptr"), feature = "default"),
    all(feature = "force_lock", not(feature = "nostd"))
))]
use std::sync::RwLock;

#[cfg(any(
    all(not(target_has_atomic = "ptr"), feature = "default"),
    all(feature = "force_lock", feature = "nostd")
))]
use spin::rwlock::RwLock;

#[cfg(any(
    all(target_has_atomic = "ptr", feature = "default"),
    all(target_has_atomic = "ptr", feature = "force_atomic")
))]
use core::sync::atomic::AtomicUsize;

const MAX_REFCOUNT: usize = (isize::MAX) as usize;

#[repr(C)]
pub struct SharedTrcInternal<T> {
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    atomicref: RwLock<usize>,
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    atomicref: AtomicUsize,
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    weakcount: RwLock<usize>,
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    weakcount: AtomicUsize,
    pub data: T,
}

/// `Trc<T>` is a heap-allocated smart pointer for sharing data across threads is a thread-safe and performant manner.
/// `Trc<T>` stands for: Thread Reference Counted.
/// `Trc<T>` provides shared ownership of the data similar to `Arc<T>` and `Rc<T>`. In addition, it also provides interior mutability.
/// It implements biased reference counting, which is based on the observation that most objects are only used by one thread.
/// This means that two reference counts can be created: one for local thread use, and one atomic one for sharing between threads.
/// This implementation of biased reference counting sets the atomic reference count to the number of threads using the data.
///
/// ## Breaking reference cycles with `Weak<T>`
/// A cycle between `Trc` pointers cannot be deallocated as the reference counts will never reach zero. The solution is a `Weak<T>`.
/// A `Weak<T>` is a non-owning reference to the data held by a `Trc<T>`.
/// They break reference cycles by adding a layer of indirection and act as an observer. They cannot even access the data directly, and
/// must be converted back into `Trc<T>`. `Weak<T>` does not keep the value alive (whcih can be dropped), and only keeps the backing allocation alive.
/// See [`Weak`] for more information.
///
/// This is relatively expensive; it allocates memory on the heap. However, calling the method
/// is most likely something that will not be done in loop.
///
/// ## Clone behavior
/// When a `Trc<T>` is cloned, it's internal (wrapped) data stays at the same memory location, but a new `Trc<T>` is constructed and returned.
/// This makes a `clone` a relatively inexpensive operation because only a wrapper is constructed.
/// This new `Trc<T>` points to the same memory, and all `Trc<T>`s that point to that memory in that thread will have their local thread reference counts incremented
/// and their atomic reference counts unchanged.
///
/// To soundly implement thread safety `Trc<T>` does not itself implement [`Send`] or [`Sync`]. However, `SharedTrc<T>` does, and it is the only way to safely send a `Trc<T>` across
/// threads. See [`SharedTrc`] for it's API, which is similar to that of `Weak`.
///
/// ## Drop behavior
///
/// When a `Trc<T>` is dropped the local thread reference count is decremented. If it is zero, the atomic reference count is also decremented.
/// If the atomic reference count is zero, then the internal data is dropped. Regardless of wherether the atomic refernce count is zero, the
/// local `Trc<T>` is dropped.
///
/// ## [`Deref`] and [`DerefMut`] behavior
/// For ease of developer use, `Trc<T>` comes with [`Deref`] implemented.
/// `Trc<T>` automatically dereferences to `&T`. This allows method calls and member acess of `T`.
/// [`DerefMut`] is not implemented as it is unsafe. However, `Trc<T>` does provide an unsafe `.deref_mut()` method to get a `&mut T`.
/// To prevent name clashes, `Trc<T>`'s functions are associated.
///
/// ## Footnote on `dyn` wrapping
/// Rust's limitations mean that `Trc` will not be able to be used as a method reciever/trait object wrapper until
/// CoerceUnsized, DispatchFromDyn, and Reciever (with arbitrary_self_types) are stablized.
/// In addition, the internal structure of `Trc<T>` means that [`NonNull`] cannot be used as an indirection for CoerceUnsized due to it's
/// internals (`*const T`), and so wrapping `dyn` types cannot be implemented. Howeover, one can use a [`Box`] as a wrapper and then wrap with `Trc<T>`.
///
/// ## Examples
///
/// Example in a single thread:
/// ```
/// use trc::Trc;
///
/// let mut trc = Trc::new(100);
/// assert_eq!(*trc, 100);
/// *unsafe { Trc::deref_mut(&mut trc)} = 200;
/// assert_eq!(*trc, 200);
/// ```
///
/// Example with multiple threads:
/// ```
/// use std::thread;
/// use trc::Trc;
/// use trc::SharedTrc;
///
/// let trc = Trc::new(100);
/// let shared = SharedTrc::from_trc(&trc);
/// let handle = thread::spawn(move || {
///     let mut trc = SharedTrc::to_trc(shared);
///     *unsafe { Trc::deref_mut(&mut trc)} = 200;
/// });
///
/// handle.join().unwrap();
/// assert_eq!(*trc, 200);
/// ```
///
pub struct Trc<T> {
    shared: NonNull<SharedTrcInternal<T>>,
    threadref: NonNull<usize>,
}

/// `SharedTrc<T>` is a thread-safe wrapper used to send `Trc<T>`s accross threads.
/// It is a wrapper around the internal state of a `Trc<T>`, and is similar to a `Weak<T>`, with the exception
/// that it does not modify the weak pointer and has less overhead. It also will not fail on conversion.
pub struct SharedTrc<T> {
    data: NonNull<SharedTrcInternal<T>>,
}

unsafe impl<T: Sync + Send> Send for SharedTrc<T> {}
unsafe impl<T: Sync + Send> Sync for SharedTrc<T> {}

impl<T> SharedTrc<T> {
    /// Convert a `Trc<T>` to a `SharedTrc<T>`, incrementing it's atomic reference count.
    /// While this `SharedTrc<T>` is alive, the data contained by `Trc<T>` will not be dropped, which is
    /// unlike a `Weak<T>`.
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    /// ```
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    pub fn from_trc(trc: &Trc<T>) -> Self {
        sum_value(&unsafe { trc.shared.as_ref() }.atomicref, 1);
        SharedTrc { data: trc.shared }
    }

    /// Convert a `Trc<T>` to a `SharedTrc<T>`, incrementing it's atomic reference count.
    /// While this `SharedTrc<T>` is alive, the data contained by `Trc<T>` will not be dropped, which is
    /// unlike a `Weak<T>`.
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    /// ```
    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    pub fn from_trc(trc: &Trc<T>) -> Self {
        sum_value(
            &unsafe { trc.shared.as_ref() }.atomicref,
            1,
            std::sync::atomic::Ordering::AcqRel,
        );
        SharedTrc { data: trc.shared }
    }

    /// Convert a `SharedTrc<T>` to a `Trc<T>`. To prevent memory leaks, this function takes
    /// ownership of the `SharedTrc`. Unlike `Weak::to_trc`, this function will not fail as it
    /// prevents the data from being dropped.
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    /// drop(trc);
    /// let trc2 = SharedTrc::to_trc(shared);
    /// ```
    pub fn to_trc(this: Self) -> Trc<T> {
        let tbx = Box::new(1);
        let res = Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: this.data,
        };
        std::mem::forget(this);
        res
    }
}

impl<T> Drop for SharedTrc<T> {
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    fn drop(&mut self) {
        use std::ptr::addr_of;

        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
        let prev = sub_value(unsafe { &(*self.data.as_ptr()).atomicref }, 1);
        let prev_weak = sub_value(unsafe { &(*self.data.as_ptr()).weakcount }, 0);

        if prev == 1 && prev_weak == 1 {
            unsafe { std::ptr::read(addr_of!((*self.data.as_ptr()).data)) };
            Weak { data: self.data };
        }
    }

    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    fn drop(&mut self) {
        use std::ptr::addr_of;

        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
        let prev = sub_value(unsafe { &(*self.data.as_ptr()).atomicref }, 1);
        let prev_weak = sub_value(unsafe { &(*self.data.as_ptr()).weakcount }, 0);

        if prev == 1 && prev_weak == 1 {
            unsafe { std::ptr::read(addr_of!((*self.data.as_ptr()).data)) };
            Weak { data: self.data };
        }
    }
}

impl<T> From<SharedTrc<T>> for Trc<T> {
    /// Convert a `SharedTrc<T>` to a `Trc<T>`. To prevent memory leaks, this function takes
    /// ownership of the `SharedTrc`. Unlike `Weak::to_trc`, this function will not fail as it
    /// prevents the data from being dropped.
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    /// drop(trc);
    /// let trc2 = SharedTrc::to_trc(shared);
    /// ```
    fn from(value: SharedTrc<T>) -> Self {
        SharedTrc::to_trc(value)
    }
}

impl<T> From<&Trc<T>> for SharedTrc<T> {
    /// Convert a `Trc<T>` to a `SharedTrc<T>`, incrementing it's atomic reference count.
    /// While this `SharedTrc<T>` is alive, the data contained by `Trc<T>` will not be dropped, which is
    /// unlike a `Weak<T>`.
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    /// ```
    fn from(value: &Trc<T>) -> Self {
        SharedTrc::from_trc(value)
    }
}

#[cfg(any(
    all(not(target_has_atomic = "ptr"), feature = "default"),
    feature = "force_lock"
))]
fn sum_value(value: &RwLock<usize>, offset: usize) -> usize {
    let mut writelock = value.try_write();

    #[cfg(not(feature = "nostd"))]
    {
        while writelock.is_err() {
            writelock = value.try_write();
        }
    }
    #[cfg(feature = "nostd")]
    {
        while writelock.is_none() {
            writelock = value.try_write();
        }
    }
    let mut writedata = writelock.unwrap();

    let res = *writedata;
    *writedata += offset;
    res
}

#[cfg(any(
    all(target_has_atomic = "ptr", feature = "default"),
    all(target_has_atomic = "ptr", feature = "force_atomic")
))]
fn sum_value(value: &AtomicUsize, offset: usize, ordering: std::sync::atomic::Ordering) -> usize {
    value.fetch_add(offset, ordering)
}

#[cfg(any(
    all(not(target_has_atomic = "ptr"), feature = "default"),
    feature = "force_lock"
))]
fn sub_value(value: &RwLock<usize>, offset: usize) -> usize {
    let mut writelock = value.try_write();

    #[cfg(not(feature = "nostd"))]
    {
        while writelock.is_err() {
            writelock = value.try_write();
        }
    }
    #[cfg(feature = "nostd")]
    {
        while writelock.is_none() {
            writelock = value.try_write();
        }
    }
    let mut writedata = writelock.unwrap();

    let res = *writedata;
    *writedata -= offset;
    res
}

#[cfg(any(
    all(target_has_atomic = "ptr", feature = "default"),
    all(target_has_atomic = "ptr", feature = "force_atomic")
))]
fn sub_value(value: &AtomicUsize, offset: usize) -> usize {
    value.fetch_sub(offset, std::sync::atomic::Ordering::AcqRel)
}

impl<T> Trc<T> {
    /// Creates a new `Trc<T>` from the provided data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// assert_eq!(*trc, 100);
    /// ```
    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    pub fn new(value: T) -> Self {
        let shareddata = SharedTrcInternal {
            atomicref: AtomicUsize::new(1),
            weakcount: AtomicUsize::new(1),
            data: value,
        };

        let sbx = Box::new(shareddata);

        let tbx = Box::new(1);

        Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: NonNull::from(Box::leak(sbx)),
        }
    }

    /// Creates a new `Trc<T>` from the provided data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// assert_eq!(*trc, 100);
    /// ```
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    pub fn new(value: T) -> Self {
        let shareddata = SharedTrcInternal {
            atomicref: RwLock::new(1),
            weakcount: RwLock::new(1),
            data: value,
        };

        let sbx = Box::new(shareddata);

        let tbx = Box::new(1);

        Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: NonNull::from(Box::leak(sbx)),
        }
    }

    /// Creates a new cyclic `Trc<T>` from the provided data. It allows the storage of `Weak<T>` which points the the allocation
    /// of `Trc<T>`inside of `T`. Holding a `Trc<T>` inside of `T` would cause a memory leak. This method works around this by
    /// providing a `Weak<T>` during the consturction of the `Trc<T>`, so that the `T` can store the `Weak<T>` internally.
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// struct T(Weak<T>);
    ///
    /// let trc = Trc::new_cyclic(|x| T(x.clone()));
    /// ```
    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    pub fn new_cyclic<F>(data_fn: F) -> Self
    where
        F: FnOnce(&Weak<T>) -> T,
    {
        let shareddata: NonNull<_> = Box::leak(Box::new(SharedTrcInternal {
            atomicref: AtomicUsize::new(0),
            weakcount: AtomicUsize::new(1),
            data: std::mem::MaybeUninit::<T>::uninit(),
        }))
        .into();

        let init_ptr: NonNull<SharedTrcInternal<T>> = shareddata.cast();

        let weak: Weak<T> = Weak { data: init_ptr };
        let data = data_fn(&weak);
        std::mem::forget(weak);

        unsafe {
            let ptr = init_ptr.as_ptr();
            std::ptr::write(std::ptr::addr_of_mut!((*ptr).data), data);
            sum_value(
                &init_ptr.as_ref().atomicref,
                1,
                std::sync::atomic::Ordering::AcqRel,
            );
        }

        let tbx = Box::new(1);

        Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: init_ptr,
        }
    }

    /// Creates a new cyclic `Trc<T>` from the provided data. It allows the storage of `Weak<T>` which points the the allocation
    /// of `Trc<T>`inside of `T`. Holding a `Trc<T>` inside of `T` would cause a memory leak. This method works around this by
    /// providing a `Weak<T>` during the consturction of the `Trc<T>`, so that the `T` can store the `Weak<T>` internally.
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// struct T(Weak<T>);
    ///
    /// let trc = Trc::new_cyclic(|x| T(x.clone()));
    /// ```
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    pub fn new_cyclic<F>(data_fn: F) -> Self
    where
        F: FnOnce(&Weak<T>) -> T,
    {
        let shareddata: NonNull<_> = Box::leak(Box::new(SharedTrcInternal {
            atomicref: RwLock::new(0),
            weakcount: RwLock::new(1),
            data: std::mem::MaybeUninit::<T>::uninit(),
        }))
        .into();

        let init_ptr: NonNull<SharedTrcInternal<T>> = shareddata.cast();

        let weak: Weak<T> = Weak { data: init_ptr };
        let data = data_fn(&weak);
        std::mem::forget(weak);

        unsafe {
            let ptr = init_ptr.as_ptr();
            std::ptr::write(std::ptr::addr_of_mut!((*ptr).data), data);
            sum_value(&init_ptr.as_ref().atomicref, 1);
        }

        let tbx = Box::new(1);

        Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: init_ptr,
        }
    }

    /// Return the local thread reference count of the object, which is how many `Trc<T>`s in this thread point to the data referenced by this `Trc<T>`.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// assert!(Trc::local_refcount(&trc) == 1)
    /// ```
    #[inline]
    pub fn local_refcount(this: &Self) -> usize {
        *unsafe { this.threadref.as_ref() }
    }

    /// Return the atomic reference count of the object. This is how many threads are using the data referenced by this `Trc<T>`.
    /// ```
    /// use std::thread;
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    ///
    /// let handle = thread::spawn(move || {
    ///     let mut trc = SharedTrc::to_trc(shared);
    ///     *unsafe { Trc::deref_mut(&mut trc)} = 200;
    ///     assert_eq!(Trc::atomic_count(&trc), 2);
    /// });
    ///
    /// handle.join().unwrap();
    /// assert_eq!(Trc::atomic_count(&trc), 1);
    /// assert_eq!(*trc, 200);
    /// ```
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    pub fn atomic_count(this: &Self) -> usize {
        let mut readlock = unsafe { this.shared.as_ref() }.atomicref.try_read();

        #[cfg(not(feature = "nostd"))]
        {
            while readlock.is_err() {
                readlock = unsafe { this.shared.as_ref() }.atomicref.try_read();
            }
        }
        #[cfg(feature = "nostd")]
        {
            while readlock.is_none() {
                readlock = unsafe { this.shared.as_ref() }.atomicref.try_read();
            }
        }
        *readlock.unwrap()
    }

    /// Return the atomic reference count of the object. This is how many threads are using the data referenced by this `Trc<T>`.
    /// ```
    /// use std::thread;
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    ///
    /// let handle = thread::spawn(move || {
    ///     let mut trc = SharedTrc::to_trc(shared);
    ///     *unsafe { Trc::deref_mut(&mut trc)} = 200;
    ///     assert_eq!(Trc::atomic_count(&trc), 2);
    /// });
    ///
    /// handle.join().unwrap();
    /// assert_eq!(Trc::atomic_count(&trc), 1);
    /// assert_eq!(*trc, 200);
    /// ```
    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    pub fn atomic_count(this: &Self) -> usize {
        unsafe { this.shared.as_ref() }
            .atomicref
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Return the weak count of the object. This is how many weak counts - across all threads - are pointing to the allocation inside of `Trc<T>`.
    /// It includes the implicit weak reference held by all Trc<T> to themselves.
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100i32);
    /// let weak = Weak::from_trc(&trc);
    /// let weak2 = Weak::from_trc(&trc);
    /// let new_trc = Weak::to_trc(&weak).expect("Value was dropped");
    /// drop(weak);
    /// assert_eq!(Trc::weak_count(&new_trc), 2);
    /// ```
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    pub fn weak_count(this: &Self) -> usize {
        let mut readlock = unsafe { this.shared.as_ref() }.weakcount.try_read();

        #[cfg(not(feature = "nostd"))]
        {
            while readlock.is_err() {
                readlock = unsafe { this.shared.as_ref() }.weakcount.try_read();
            }
        }
        #[cfg(feature = "nostd")]
        {
            while readlock.is_none() {
                readlock = unsafe { this.shared.as_ref() }.weakcount.try_read();
            }
        }
        *readlock.unwrap()
    }

    /// Return the weak count of the object. This is how many weak counts - across all threads - are pointing to the allocation inside of `Trc<T>`.
    /// It includes the implicit weak reference held by all Trc<T> to themselves.
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100i32);
    /// let weak = Weak::from_trc(&trc);
    /// let weak2 = Weak::from_trc(&trc);
    /// let new_trc = Weak::to_trc(&weak).expect("Value was dropped");
    /// drop(weak);
    /// assert_eq!(Trc::weak_count(&new_trc), 2);
    /// ```
    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    pub fn weak_count(this: &Self) -> usize {
        unsafe { this.shared.as_ref() }
            .weakcount
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Checks if the other `Trc<T>` is equal to this one according to their internal pointers.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc1 = Trc::new(100);
    /// let trc2 = trc1.clone();
    /// assert!(Trc::ptr_eq(&trc1, &trc2));
    /// ```
    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.shared.as_ptr() == other.shared.as_ptr()
    }

    /// Gets the raw pointer to the most inner layer of `Trc<T>`.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// println!("{}", Trc::as_ptr(&trc) as usize)
    /// ```
    #[inline]
    pub fn as_ptr(this: &Self) -> *mut SharedTrcInternal<T> {
        this.shared.as_ptr()
    }

    /// Creates a new `Pin<Trc<T>>`. If `T` does not implement [`Unpin`], then the data will be pinned in memory and unable to be moved.
    #[inline]
    pub fn pin(data: T) -> Pin<Trc<T>> {
        unsafe { Pin::new_unchecked(Trc::new(data)) }
    }

    /// Get a &mut reference to the internal data.
    ///
    /// # Safety
    /// This function is unsafe because it can open up the possibility of UB if the programmer uses it
    /// improperly in a threaded enviornment, tht is if there are concurrent writes.
    ///
    /// - While this reference exists, there must be no other reads or writes.
    ///
    /// ```
    /// use trc::Trc;
    /// use std::ops::DerefMut;
    ///
    /// let mut trc = Trc::new(100);
    /// *unsafe { Trc::deref_mut(&mut trc)} = 200;
    /// let mutref = unsafe { trc.deref_mut() };
    /// *mutref = 300;
    /// assert_eq!(*trc, 300);
    /// ```
    #[inline]
    pub unsafe fn deref_mut(&mut self) -> &mut T {
        &mut self.shared.as_mut().data
    }
}

impl<T> Deref for Trc<T> {
    type Target = T;

    /// Get an immutable reference to the internal data.
    /// ```
    /// use trc::Trc;
    /// use std::ops::Deref;
    ///
    /// let mut trc = Trc::new(100i32);
    /// assert_eq!(*trc, 100i32);
    /// assert_eq!(trc.deref(), &100i32);
    /// ```
    #[inline]
    fn deref(&self) -> &Self::Target {
        &unsafe { self.shared.as_ref() }.data
    }
}

impl<T> Drop for Trc<T> {
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    fn drop(&mut self) {
        use std::ptr::addr_of;

        *unsafe { self.threadref.as_mut() } -= 1;

        if *unsafe { self.threadref.as_ref() } == 0 {
            let prev = sub_value(&unsafe { self.shared.as_ref() }.atomicref, 1);

            if prev == 1 {
                unsafe { std::ptr::read(addr_of!((*self.shared.as_ptr()).data)) };
                Weak { data: self.shared };
            }
            unsafe { Box::from_raw(self.threadref.as_ptr()) };
        }
    }

    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    fn drop(&mut self) {
        use std::ptr::addr_of;

        *unsafe { self.threadref.as_mut() } -= 1;
        if *unsafe { self.threadref.as_ref() } == 0 {
            let prev = sub_value(&unsafe { self.shared.as_ref() }.atomicref, 1);
            if prev == 1 {
                unsafe { std::ptr::read(addr_of!((*self.shared.as_ptr()).data)) };
                Weak { data: self.shared };
            }
            unsafe { Box::from_raw(self.threadref.as_ptr()) };
        }
    }
}

impl<T> Clone for Trc<T> {
    /// Clone a `Trc<T>` (increment it's local reference count). This can only be used to clone an object that will only stay in one thread.
    /// It will panic if the local reference count overflows.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// let trc2 = trc.clone();
    /// assert_eq!(Trc::local_refcount(&trc), Trc::local_refcount(&trc2));
    /// ```
    #[inline]

    fn clone(&self) -> Self {
        unsafe { *self.threadref.as_ptr() += 1 }

        Trc {
            shared: self.shared,
            threadref: self.threadref,
        }
    }
}

impl<T> AsRef<T> for Trc<T> {
    fn as_ref(&self) -> &T {
        Trc::deref(self)
    }
}

impl<T> Borrow<T> for Trc<T> {
    fn borrow(&self) -> &T {
        self.as_ref()
    }
}

impl<T: Default> Default for Trc<T> {
    fn default() -> Self {
        Trc::new(Default::default())
    }
}

impl<T: Display> Display for Trc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt((*self).deref(), f)
    }
}

impl<T: Debug> Debug for Trc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt((*self).deref(), f)
    }
}

impl<T: Pointer> Pointer for Trc<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Pointer::fmt((*self).deref(), f)
    }
}

impl<T> From<T> for Trc<T> {
    /// Create a new `Trc<T>` from the provided data. This is equivalent to calling `Trc::new` on the same data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::from(100);
    /// assert_eq!(*trc, 100);
    /// ```
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Hash> Hash for Trc<T> {
    /// Pass the data contained in this `Trc<T>` to the provided hasher.
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl<T: PartialOrd> PartialOrd for Trc<T> {
    /// "Greater than or equal to" comparison for two `Trc<T>`s.
    ///
    /// Calls `.partial_cmp` on the data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc1 = Trc::from(100);
    /// let trc2 = Trc::from(100);
    /// assert!(trc1>=trc2);
    /// ```
    #[inline]
    fn ge(&self, other: &Self) -> bool {
        self.deref().ge(other.deref())
    }

    /// "Less than or equal to" comparison for two `Trc<T>`s.
    ///
    /// Calls `.le` on the data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc1 = Trc::from(100);
    /// let trc2 = Trc::from(100);
    /// assert!(trc1<=trc2);
    /// ```
    #[inline]
    fn le(&self, other: &Self) -> bool {
        self.deref().ge(other.deref())
    }

    /// "Greater than" comparison for two `Trc<T>`s.
    ///
    /// Calls `.gt` on the data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc1 = Trc::from(200);
    /// let trc2 = Trc::from(100);
    /// assert!(trc1>trc2);
    /// ```
    #[inline]
    fn gt(&self, other: &Self) -> bool {
        self.deref().gt(other.deref())
    }

    /// "Less than" comparison for two `Trc<T>`s.
    ///
    /// Calls `.lt` on the data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc1 = Trc::from(100);
    /// let trc2 = Trc::from(200);
    /// assert!(trc1<trc2);
    /// ```
    #[inline]
    fn lt(&self, other: &Self) -> bool {
        self.deref().lt(other.deref())
    }

    /// Partial comparison for two `Trc<T>`s.
    ///
    /// Calls `.partial_cmp` on the data.
    /// ```
    /// use trc::Trc;
    /// use std::cmp::Ordering;
    ///
    /// let trc1 = Trc::from(100);
    /// let trc2 = Trc::from(200);
    /// assert_eq!(Some(Ordering::Less), trc1.partial_cmp(&trc2));
    /// ```
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<T: Ord> Ord for Trc<T> {
    /// Create a new `Trc<T>` from the provided data. This is equivalent to calling `Trc::new` on the same data.
    /// ```
    /// use trc::Trc;
    /// use std::cmp::Ordering;
    ///
    /// let trc1 = Trc::from(100);
    /// let trc2 = Trc::from(200);
    /// assert_eq!(Ordering::Less, trc1.cmp(&trc2));
    /// ```
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.deref().cmp(other.deref())
    }
}

impl<T: Eq> Eq for Trc<T> {}

impl<T: PartialEq> PartialEq for Trc<T> {
    /// Equality by value comparison for two `Trc<T>`s, even if the data is in different allocoations.
    ///
    /// Calls `.eq` on the data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc1 = Trc::from(100);
    /// let trc2 = Trc::from(100);
    /// assert!(trc1==trc2);
    /// ```
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }

    /// Equality by value comparison for two `Trc<T>`s, even if the data is in different allocoations.
    ///
    /// Calls `.ne` on the data.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc1 = Trc::from(100);
    /// let trc2 = Trc::from(200);
    /// assert!(trc1!=trc2);
    /// ```
    #[allow(clippy::partialeq_ne_impl)]
    #[inline]
    fn ne(&self, other: &Self) -> bool {
        self.deref().ne(other.deref())
    }
}

/// `Weak<T>` is a non-owning reference to `Trc<T>`'s data. It is used to prevent cyclic references which cause memory to never be freed.
/// `Weak<T>` does not keep the value alive (which can be dropped), they only keep the backing allocation alive. `Weak<T>` cannot even directly access the memory,
/// and must be converted into `Trc<T>` to do so.
///
/// One use case of a `Weak<T>`
/// is to create a tree: The parent nodes own the child nodes, and have strong `Trc<T>` references to their children. However, their children have `Weak<T>` references
/// to their parents.
///
/// To prevent name clashes, `Weak<T>`'s functions are associated.
///
/// # Examples
///
/// Example in a single thread:
/// ```
/// use trc::Trc;
/// use trc::Weak;
///
/// let trc = Trc::new(100);
/// let weak = Weak::from_trc(&trc);
/// let mut new_trc = Weak::to_trc(&weak).unwrap();
/// assert_eq!(*new_trc, 100);
/// *unsafe { Trc::deref_mut(&mut new_trc)} = 200;
/// assert_eq!(*new_trc, 200);
/// ```
///
/// Example with multiple threads:
/// ```
/// use std::thread;
/// use trc::Trc;
/// use trc::trc::SharedTrc;
///
/// let trc = Trc::new(100);
/// let shared = SharedTrc::from_trc(&trc);
/// let handle = thread::spawn(move || {
///     let mut trc = SharedTrc::to_trc(shared);
///     assert_eq!(*trc, 100);
///     *unsafe { Trc::deref_mut(&mut trc)} = 200;
/// });
/// handle.join().unwrap();
/// assert_eq!(*trc, 200);
/// ```
///
pub struct Weak<T> {
    data: NonNull<SharedTrcInternal<T>>, //Use this data because it has the ptr
}

impl<T> Drop for Weak<T> {
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    fn drop(&mut self) {
        use std::alloc::Layout;
        let prev = sub_value(unsafe { &(*self.data.as_ptr()).weakcount }, 1);

        let mut readlock = unsafe { &(*self.data.as_ptr()).atomicref }.try_read();

        #[cfg(not(feature = "nostd"))]
        {
            while readlock.is_err() {
                readlock = unsafe { &(*self.data.as_ptr()).atomicref }.try_read();
            }
        }
        #[cfg(feature = "nostd")]
        {
            while readlock.is_none() {
                readlock = unsafe { &(*self.data.as_ptr()).atomicref }.try_read();
            }
        }
        let atomicdata = (*readlock.as_ref().unwrap()).clone();
        drop(readlock);
        if prev == 1 && atomicdata == 0 {
            let (size, align) = (
                std::mem::size_of::<SharedTrcInternal<T>>(),
                std::mem::align_of::<SharedTrcInternal<T>>(),
            );
            let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
            unsafe { std::alloc::dealloc(self.data.as_ptr().cast(), layout) };
        }
    }

    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    fn drop(&mut self) {
        use std::alloc::Layout;
        let prev = sub_value(unsafe { &(*self.data.as_ptr()).weakcount }, 1);

        let atomic =
            unsafe { &(*self.data.as_ptr()).atomicref }.load(std::sync::atomic::Ordering::Acquire);

        if prev == 1 && atomic == 0 {
            let (size, align) = (
                std::mem::size_of::<SharedTrcInternal<T>>(),
                std::mem::align_of::<SharedTrcInternal<T>>(),
            );
            let layout = unsafe { Layout::from_size_align_unchecked(size, align) };
            unsafe { std::alloc::dealloc(self.data.as_ptr().cast(), layout) };
        }
    }
}

impl<T> Weak<T> {
    /// Create a `Weak<T>` from a `Trc<T>`. This increments the weak count.
    ///
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let weak = Weak::from_trc(&trc);
    /// ```
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    pub fn from_trc(trc: &Trc<T>) -> Self {
        sum_value(&unsafe { trc.shared.as_ref() }.weakcount, 1);
        Weak { data: trc.shared }
    }

    /// Create a `Weak<T>` from a `Trc<T>`. This increments the weak count.
    ///
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let weak = Weak::from_trc(&trc);
    /// ```
    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    pub fn from_trc(trc: &Trc<T>) -> Self {
        sum_value(
            &unsafe { trc.shared.as_ref() }.weakcount,
            1,
            std::sync::atomic::Ordering::AcqRel,
        );
        Weak { data: trc.shared }
    }

    /// Create a `Trc<T>` from a `Weak<T>`. Because `Weak<T>` does not own the value, it might have been dropped already. If it has, a `None` is returned.
    /// If the value has not been dropped, then this function a) decrements the weak count, and b) increments the atomic reference count of the object.
    ///
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100i32);
    /// let weak = Weak::from_trc(&trc);
    /// let new_trc = Weak::to_trc(&weak).expect("Value was dropped");
    /// drop(weak);
    /// assert_eq!(*new_trc, 100i32);
    /// ```
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    pub fn to_trc(this: &Self) -> Option<Trc<T>> {
        let mut writelock = unsafe { this.data.as_ref() }.atomicref.try_write();

        #[cfg(not(feature = "nostd"))]
        {
            while writelock.is_err() {
                writelock = unsafe { this.data.as_ref() }.atomicref.try_write();
            }
        }
        #[cfg(feature = "nostd")]
        {
            while writelock.is_none() {
                writelock = unsafe { this.data.as_ref() }.atomicref.try_write();
            }
        }
        let mut writedata = writelock.unwrap();

        if *writedata == 0 {
            return None;
        }

        *writedata += 1;

        let tbx = Box::new(1);

        Some(Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: this.data,
        })
    }

    /// Create a `Trc<T>` from a `Weak<T>`. Because `Weak<T>` does not own the value, it might have been dropped already. If it has, a `None` is returned.
    /// If the value has not been dropped, then this function a) decrements the weak count, and b) increments the atomic reference count of the object.
    ///
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100i32);
    /// let weak = Weak::from_trc(&trc);
    /// let new_trc = Weak::to_trc(&weak).expect("Value was dropped");
    /// drop(weak);
    /// assert_eq!(*new_trc, 100i32);
    /// ```
    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    pub fn to_trc(this: &Self) -> Option<Trc<T>> {
        if unsafe { this.data.as_ref() }
            .atomicref
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            return None;
        }

        sum_value(
            &unsafe { this.data.as_ref() }.atomicref,
            1,
            std::sync::atomic::Ordering::AcqRel,
        );

        let tbx = Box::new(1);

        Some(Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: this.data,
        })
    }
}

impl<T> Clone for Weak<T> {
    /// Clone a `Weak<T>` (increment the weak count).
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let weak1 = Weak::from_trc(&trc);
    /// let weak2 = weak1.clone();
    /// assert_eq!(Trc::weak_count(&trc), 3);
    /// ```
    #[inline]
    #[cfg(any(
        all(not(target_has_atomic = "ptr"), feature = "default"),
        feature = "force_lock"
    ))]
    fn clone(&self) -> Self {
        let prev = sum_value(&unsafe { self.data.as_ref() }.weakcount, 1);

        if prev > MAX_REFCOUNT {
            panic!("Overflow of maximum weak reference count.");
        }

        Weak { data: self.data }
    }

    /// Clone a `Weak<T>` (increment the weak count).
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let weak1 = Weak::from_trc(&trc);
    /// let weak2 = weak1.clone();
    /// assert_eq!(Trc::weak_count(&trc), 3);
    /// ```
    #[inline]
    #[cfg(any(
        all(target_has_atomic = "ptr", feature = "default"),
        all(target_has_atomic = "ptr", feature = "force_atomic")
    ))]
    fn clone(&self) -> Self {
        let prev = sum_value(
            &unsafe { self.data.as_ref() }.weakcount,
            1,
            std::sync::atomic::Ordering::Relaxed,
        );

        if prev > MAX_REFCOUNT {
            panic!("Overflow of maximum weak reference count.");
        }

        Weak { data: self.data }
    }
}

unsafe impl<T: Sync + Send> Send for Weak<T> {}
unsafe impl<T: Sync + Send> Sync for Weak<T> {}
