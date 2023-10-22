//! `Trc` is a performant heap-allocated smart pointer that implements a thread reference counting.
//! `Trc` stands for: Thread Reference Counted.
//! [`Trc`] provides shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
//! It implements thread reference counting, which is based on the observation that most objects are only used by one thread.
//! This means that two reference counts can be created: one for local thread use, and one atomic one for sharing between threads.
//! Thread reference counting sets the atomic reference count to the number of threads using the data.
//!
//! ## Breaking reference cycles with [`Weak<T>`]
//! A cycle between `Trc` pointers cannot be deallocated as the reference counts will never reach zero. The solution is a `Weak<T>`.
//! A `Weak` is a non-owning reference to the data held by a `Trc`.
//! They break reference cycles by adding a layer of indirection and act as an observer. They cannot access the data directly, and
//! must be converted back into `Trc`. `Weak` does not keep the value alive (which can be dropped), and only keeps the backing allocation alive.
//!
//! ## Sending data across threads with [`SharedTrc<T>`]
//! To soundly implement thread safety `Trc<T>` is `!Send` and `!Sync`.
//! To solve this, `Trc` introduces a `SharedTrc<T>`, which is [`Send`] and [`Sync`].
//! `SharedTrc` is the only way to safely send a `Trc`'s data across threads without using a `Weak`.
//!
//! Because `Trc` is not part of the standard library,
//! the `CoerceUnsized` and `Receiver` traits cannot currently be implemented by default.
//! However, `Trc` provides `dyn_unstable` trait which enables the above traits for
//! `Trc` and `SharedTrc` and must be used with nightly Rust (`cargo +nightly ...`).

#![cfg_attr(feature = "dyn_unstable", feature(unsize))]
#![cfg_attr(feature = "dyn_unstable", feature(coerce_unsized))]
#![cfg_attr(feature = "dyn_unstable", feature(receiver_trait))]
#![cfg_attr(feature = "dyn_unstable", feature(dispatch_from_dyn))]

#[deny(clippy::all)]
#[cfg(test)]
mod tests;

#[cfg(not(target_has_atomic = "ptr"))]
compile_error!("Cannot use `Trc` on a system without atomics.");

use std::{
    alloc::{alloc, Layout},
    borrow::Borrow,
    error::Error,
    fmt::{Debug, Display, Pointer},
    hash::{Hash, Hasher},
    mem::{forget, ManuallyDrop, MaybeUninit},
    ops::Deref,
    panic::UnwindSafe,
    pin::Pin,
    ptr::{self, addr_of, addr_of_mut, slice_from_raw_parts_mut, write, NonNull},
};

#[cfg(not(target_os = "windows"))]
use std::os::fd::{AsFd, AsRawFd};

#[cfg(target_os = "windows")]
use std::os::windows::io::{AsHandle, AsRawHandle, AsRawSocket, AsSocket};

#[cfg(feature = "dyn_unstable")]
use std::any::Any;

use core::sync::atomic::AtomicUsize;

const MAX_REFCOUNT: usize = (isize::MAX) as usize;

#[repr(C)]
struct SharedTrcInternal<T: ?Sized> {
    atomicref: AtomicUsize,
    weakcount: AtomicUsize,
    data: T,
}

/// `Trc` is a performant heap-allocated smart pointer that implements thread reference counting.
/// `Trc` stands for: Thread Reference Counted.
/// `Trc` provides shared ownership of the data similar to `Arc<T>` and `Rc<T>`.
/// It implements thread reference counting, which is based on the observation that most objects are only used by one thread.
/// This means that two reference counts can be created: one for local thread use, and one atomic one for sharing between threads.
/// Thread reference counting sets the atomic reference count to the number of threads using the data.
///
/// ## Construction behavior
/// A `Trc` can be constructed via several methods, and even from a `SharedTrc` or `Weak`. When a `Trc` is created, memory is allocated
/// and the atomic reference and weak reference counts are both set to 1 (with the exception of `Weak::upgrade`). All `Trc`s together
/// have an implicit weak reference to themselves, and thus the weak reference count is always at least 1.
///
/// ## Clone behavior
/// When a `Trc` is cloned, it's internal (wrapped) data is not cloned. Instead, a new `Trc` that point to the data is constructed and returned.
/// This makes a `clone` a relatively inexpensive operation because only a wrapper is constructed.
/// All `Trc`s that point to the data in the thread will have their local thread reference counts incremented, with their atomic reference counts unchanged.
///
/// ## Drop behavior
/// When a `Trc` is dropped the local thread reference count is decremented. If it is zero, the atomic reference count is also decremented.
/// If the atomic reference count and weak reference count are both zero, only then the memory freed.
///
/// ## [`Deref`] behavior
/// For ease of developer use, `Trc` implements [`Deref`].
/// `Trc` automatically dereferences to `&T`. This allows method calls and member access of `T`.
/// To prevent name clashes, `Trc<T>`'s methods are associated.
///
/// ## Trait object behavior and limitations
/// Because `Trc` is not in the standard library, it cannot implement the `CoerceUnsized` or `Receiever` traits by default in stable Rust.
/// However, `Trc` has a feature `dyn_unstable` that enables these features to be implemented for `Trc` and allow coercion to trait objects
/// (`Trc<dyn T>`) as well as acting as a method receiver (`fn _(&self)`). Unfortunately, because of the internal design of `Trc`, `DispatchFromDyn`
/// cannot be implemented (so `fn _(self: Trc<Self>)` cannot be implemented). However, [`SharedTrc`] does implement `DispatchFromDyn`.
///
/// Similarly, because of the design of `Trc`, it would be unsound to include the `into/from_raw` or `increment/decrement_local_count` methods.
///
/// ## Examples
///
/// Example in a single thread:
/// ```
/// use trc::Trc;
///
/// let mut trc = Trc::new(100);
/// assert_eq!(*trc, 100);
/// *Trc::get_mut(&mut trc).unwrap() = 200;
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
/// });
///
/// handle.join().unwrap();
/// assert_eq!(*trc, 100);
/// ```
///
pub struct Trc<T: ?Sized> {
    shared: NonNull<SharedTrcInternal<T>>,
    threadref: NonNull<usize>,
}

/// `SharedTrc` is a thread-safe wrapper used to send `Trc`s across threads.
/// Unlike [`Trc`] (which is `!Send` and `!Sync`), `SharedTrc` is [`Send`] and [`Sync`]. This means that along with
/// [`Weak`], `SharedTrc` is one of the ways to send a `Trc` across threads. However, unlike `Weak`, `SharedTrc` does not
/// modify the weak count - and only modifies the atomic count. In addition, `SharedTrc` will not fail on conversion
/// back to a `Trc` because it prevents the data `T` from being dropped.
///
/// ## Construction behavior
/// A `SharedTrc` can be constructed explicitly using methods or using the `Into` trait. When a `SharedTrc` is constructed,
/// no memory is allocated. However, the atomic reference count is incremented, has a small overhead. All `SharedTrc`s together
/// have an implicit weak reference to themselves, and thus the weak reference count is always at least 1.
///
/// ## Clone behavior
/// When a `SharedTrc` is cloned, it's internal (wrapped) data is not cloned. Instead, a new `Trc` that points to the data is constructed and returned.
/// In contrast with `Trc`, `SharedTrc` uses an atomic operation to increment the atomic reference count.
/// This gives [`SharedTrc::clone`] move overhead than [`Trc::clone`].
///
/// ## Drop behavior
/// When a `SharedTrc` is dropped the atomic reference count is decremented.
/// If the atomic reference count and weak reference count are both zero, only then the memory freed.
///
/// ## [`Deref`] behavior
/// For ease of developer use, `SharedTrc` implements [`Deref`].
/// `SharedTrc` automatically dereferences to `&T`. This allows method calls and member access of `T`.
/// To prevent name clashes, `SharedTrc<T>`'s methods are associated.
///
/// ## Trait object behavior and limitations
/// Because `SharedTrc` is not in the standard library, it cannot implement the `CoerceUnsized`, `DispatchFromDyn` or `Receiever` traits by default in stable Rust.
/// However, `Trc` has a feature `dyn_unstable` that enables these features to be implemented for `SharedTrc` and allow coercion to trait objects
/// (`SharedTrc<dyn T>`) as well as acting as a method receiver (`fn _(&self)`) and allowing trait-object safety with arbitrary self types (`fn _(self: Trc<Self>)`).
///
/// ## Examples
///
/// Example in a single thread:
/// ```
/// use trc::Trc;
/// use trc::SharedTrc;
///
/// let trc = Trc::new(String::from("Trc"));
/// let shared: SharedTrc<_> = (&trc).into();
/// let trc2: Trc<String> = shared.into();
/// assert_eq!(*trc, *trc2);
/// ```
///
/// See [`Trc`] or [`Weak`] for an example with multiple threads.
pub struct SharedTrc<T: ?Sized> {
    data: NonNull<SharedTrcInternal<T>>,
}

/// `Weak` is a non-owning reference to `Trc`'s data. It is used to prevent cyclic references which cause memory to never be freed.
/// `Weak` does not keep the value alive (which can be dropped), they only keep the backing allocation alive. `Weak` cannot directly access the data,
/// and must be converted into `Trc` to do so.
///
/// ## Construction behavior
/// A `Weak` is constructed from a `Trc`, and no memory is allocated.
/// However, the weak reference count is incremented, which has a small overhead.
///
/// ## Clone behavior
/// When a `Weak` is cloned, a new `Weak` that may point to the data is constructed and returned.
/// In contrast with `Trc`, `Weak` uses an atomic operation to increment the weak reference count.
/// This gives [`Weak::clone`] move overhead than [`Trc::clone`].
///
/// ## Drop behavior
/// When a `Weak` is dropped the weak reference count is decremented.
/// If the atomic reference count and weak reference count are both zero, only then the memory freed.
///
/// One use case of a `Weak` is to create a tree:
/// The parent nodes own the child nodes, and have strong `Trc` references to their children.
/// However, their children have `Weak` references to their parents.
///
/// To prevent name clashes, `Weak<T>`'s functions are associated.
///
/// ## Examples
///
/// Example in a single thread:
/// ```
/// use trc::Trc;
/// use trc::Weak;
///
/// let trc = Trc::new(100);
/// let weak = Trc::downgrade(&trc);
/// let new_trc = Weak::upgrade(&weak).unwrap();
/// assert_eq!(*new_trc, 100);
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
///     assert_eq!(*trc, 100);
/// });
/// handle.join().unwrap();
/// assert_eq!(*trc, 100);
/// ```
///
pub struct Weak<T: ?Sized> {
    data: NonNull<SharedTrcInternal<T>>,
}

impl<T: ?Sized> SharedTrc<T> {
    /// Construct a `SharedTrc` from a `Trc`, incrementing it's atomic reference count.
    /// While this `SharedTrc` is alive, the data contained by `Trc` will not be dropped, which is
    /// unlike a `Weak`.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    /// ```
    #[inline]
    pub fn from_trc(trc: &Trc<T>) -> Self {
        let prev = sum_value(
            &unsafe { trc.shared.as_ref() }.atomicref,
            1,
            core::sync::atomic::Ordering::Acquire,
        );
        if prev > MAX_REFCOUNT {
            panic!("Overflow of maximum atomic reference count.");
        }
        SharedTrc { data: trc.shared }
    }

    /// Convert a `SharedTrc` to a `Trc`. To prevent memory leaks, this function takes
    /// ownership of the `SharedTrc`. Unlike [`Trc::upgrade`], this function will not fail as it
    /// prevents the data from being dropped.
    ///
    /// # Examples
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
        core::mem::forget(this);
        res
    }

    /// Return the atomic reference count of the object. This is how many threads are using the data referenced by this `SharedTrc`.
    ///
    /// # Examples
    /// ```
    /// use std::thread;
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    ///
    /// let handle = thread::spawn(move || {
    ///     assert_eq!(SharedTrc::atomic_count(&shared), 2);
    ///     let trc = SharedTrc::to_trc(shared);
    /// });
    ///
    /// handle.join().unwrap();
    /// assert_eq!(*trc, 100);
    /// ```
    #[inline]
    pub fn atomic_count(this: &Self) -> usize {
        unsafe { this.data.as_ref() }
            .atomicref
            .load(core::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(feature = "dyn_unstable")]
impl SharedTrc<dyn Any + Send + Sync> {
    /// Attempts to downcast a `SharedTrc<dyn Any + Send + Sync>` into a concrete type.
    ///
    /// # Examples
    /// ```
    /// use std::any::Any;
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// fn print_if_string(value: SharedTrc<dyn Any + Send + Sync>) {
    ///     if let Ok(string) = value.downcast::<String>() {
    ///         println!("String ({}): {}", string.len(), string);
    ///     }
    /// }
    ///
    /// let my_string = "Hello World".to_string();
    /// let a: Trc<dyn Any + Send + Sync> = Trc::new(my_string);
    /// let b: Trc<dyn Any + Send + Sync> = Trc::new(0i8);
    /// print_if_string(SharedTrc::from_trc(&a));
    /// print_if_string(SharedTrc::from_trc(&b));
    /// ```
    pub fn downcast<T>(self) -> Result<SharedTrc<T>, Self>
    where
        T: Any + Send + Sync,
    {
        if (*self).is::<T>() {
            let data = self.data.cast::<SharedTrcInternal<T>>();
            forget(self);
            Ok(SharedTrc { data })
        } else {
            Err(self)
        }
    }
}

impl<T: ?Sized> Clone for SharedTrc<T> {
    /// Clone a `SharedTrc` (increment the atomic count).
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared1 = SharedTrc::from_trc(&trc);
    /// let shared2 = shared1.clone();
    /// assert_eq!(SharedTrc::atomic_count(&shared1), 3);
    /// ```
    #[inline]
    fn clone(&self) -> Self {
        let prev = sum_value(
            &unsafe { self.data.as_ref() }.atomicref,
            1,
            core::sync::atomic::Ordering::AcqRel,
        );
        if prev > MAX_REFCOUNT {
            panic!("Overflow of maximum atomic reference count.");
        }
        SharedTrc { data: self.data }
    }
}

impl<T: ?Sized> Drop for SharedTrc<T> {
    #[inline]
    fn drop(&mut self) {
        if sub_value(
            unsafe { &(*self.data.as_ptr()).atomicref },
            1,
            core::sync::atomic::Ordering::Release,
        ) != 1
        {
            return;
        }

        let weak =
            unsafe { &(*self.data.as_ptr()).weakcount }.load(core::sync::atomic::Ordering::Acquire);
        if weak == 1 {
            core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
            unsafe { core::ptr::drop_in_place(addr_of_mut!((*self.data.as_ptr()).data)) };
            Weak { data: self.data };
        }
    }
}

impl<T: ?Sized> From<SharedTrc<T>> for Trc<T> {
    /// Convert a `SharedTrc` to a `Trc`. To prevent memory leaks, this function takes
    /// ownership of the `SharedTrc`. Unlike [`Weak::to_trc`], this function will not fail as it
    /// prevents the data from being dropped.
    ///
    /// # Examples
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

impl<T: ?Sized> From<&Trc<T>> for SharedTrc<T> {
    /// Convert a `Trc` to a `SharedTrc`, incrementing it's atomic reference count.
    /// While this `SharedTrc<T>` is alive, the data contained by `Trc<T>` will not be dropped, which is
    /// unlike a `Weak<T>`.
    ///
    /// # Examples
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

impl<T: ?Sized> From<Trc<T>> for SharedTrc<T> {
    /// Convert a `Trc<T>` to a `SharedTrc<T>`, incrementing it's atomic reference count.
    /// While this `SharedTrc<T>` is alive, the data contained by `Trc<T>` will not be dropped, which is
    /// unlike a `Weak<T>`.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    /// ```
    fn from(value: Trc<T>) -> Self {
        SharedTrc::from_trc(&value)
    }
}

impl<T: ?Sized> SharedTrc<T> {
    /// Return the weak count of the object. This is how many weak counts - across all threads - are pointing to the allocation inside of `SharedTrc`.
    /// It includes the implicit weak reference held by all `Trc` or `SharedTrc` to themselves.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100i32);
    /// let weak = Trc::downgrade(&trc);
    /// let weak2 = Trc::downgrade(&trc);
    /// let new_trc = Weak::upgrade(&weak).expect("Value was dropped");
    /// drop(weak);
    /// let shared: SharedTrc<_> = trc.into();
    /// assert_eq!(SharedTrc::weak_count(&shared), 2);
    /// ```
    #[inline]
    pub fn weak_count(this: &Self) -> usize {
        unsafe { this.data.as_ref() }
            .weakcount
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    /// Checks if the other `SharedTrc` is equal to this one according to their internal pointers.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc1 = Trc::new(100);
    /// let trc2 = trc1.clone();
    /// let shared1: SharedTrc<_> = trc1.into();
    /// let shared2: SharedTrc<_> = trc2.into();
    /// assert!(SharedTrc::ptr_eq(&shared1, &shared2));
    /// ```
    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.data.as_ptr() == other.data.as_ptr()
    }

    /// Gets the raw pointer to the most inner layer of `SharedTrc`.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let trc = Trc::new(100);
    /// assert_eq!(SharedTrc::as_ptr(&SharedTrc::from_trc(&trc)), Trc::as_ptr(&trc))
    /// ```
    #[inline]
    pub fn as_ptr(this: &Self) -> *const T {
        let sharedptr = NonNull::as_ptr(this.data);
        unsafe { addr_of_mut!((*sharedptr).data) }
    }

    /// Converts a `SharedTrc` into `*const T`, without freeing the allocation.
    /// To avoid a memory leak, be sure to call [`SharedTrc::from_raw`] to reclaim the allocation.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared: SharedTrc<_> = Trc::new(100).into();
    /// let ptr = SharedTrc::into_raw(shared);
    ///
    /// assert_eq!(unsafe { *ptr }, 100);
    ///
    /// unsafe { SharedTrc::from_raw(ptr) };
    /// ```
    pub fn into_raw(this: Self) -> *const T {
        let ptr = Self::as_ptr(&this);

        forget(this);
        ptr
    }
}

impl<T> SharedTrc<T> {
    /// Converts a `*const T` into `SharedTrc`. The caller must uphold the below safety constraints.
    ///
    /// # Safety
    /// - The given pointer must be a valid pointer to `T` that came from [`SharedTrc::into_raw`].
    /// - After `from_raw`, the pointer must not be accessed.
    ///
    /// # Examples
    /// Example 1:
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared: SharedTrc<_> = Trc::new(100).into();
    /// let ptr = SharedTrc::into_raw(shared);
    ///
    /// assert_eq!(unsafe { *ptr }, 100);
    ///
    /// unsafe { SharedTrc::from_raw(ptr) };
    ///
    /// ```
    ///
    /// Example 2:
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    /// use trc::SharedTrc;
    ///
    /// let strong = Trc::new("hello".to_owned());
    ///
    /// let raw_1 = SharedTrc::into_raw(SharedTrc::from_trc(&strong));
    /// let raw_2 = SharedTrc::into_raw(SharedTrc::from_trc(&strong));
    ///
    /// assert_eq!(3, Trc::atomic_count(&strong));
    ///
    /// assert_eq!("hello", &*SharedTrc::to_trc(unsafe { SharedTrc::from_raw(raw_1) }));
    /// assert_eq!(2, Trc::atomic_count(&strong));
    ///
    /// drop(strong);
    ///
    /// // Decrement the last atomic count.
    /// SharedTrc::to_trc(unsafe { SharedTrc::from_raw(raw_2) });
    /// ```
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        let layout = Layout::new::<SharedTrcInternal<()>>();
        let n = layout.size();

        let data_ptr = (ptr as *const u8).sub(n) as *mut SharedTrcInternal<T>;

        SharedTrc {
            data: NonNull::new_unchecked(data_ptr),
        }
    }

    /// Decrements the local reference count of the provided `SharedTrc` associated with the provided pointer.
    /// If the local count is 1, then the atomic count will also be decremented. If the atomic count is 0, the value will be dropped.
    ///
    /// # Safety
    /// - The provided pointer must have been obtained through `SharedTrc::from_raw`.
    /// - The atomic count must be at least 1 throughout the duration of this method.
    /// - This method **should not** be called after the final `Trc` or `SharedTrc` has been released.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared: SharedTrc<_> = Trc::new(100).into();
    /// let ptr = SharedTrc::into_raw(shared);
    ///
    /// assert_eq!(unsafe { *ptr }, 100);
    ///
    /// unsafe { SharedTrc::decrement_local_count(ptr) };
    /// ```
    ///
    pub unsafe fn decrement_local_count(ptr: *const T) {
        drop(SharedTrc::from_raw(ptr));
    }

    /// Increments the local reference count of the provided `SharedTrc` associated with the provided pointer.
    ///
    /// # Safety
    /// - The provided pointer must have been obtained through `SharedTrc::from_raw`.
    /// - The atomic count must be at least 1 throughout the duration of this method.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared: SharedTrc<_> = Trc::new(100).into();
    /// let shared2 = shared.clone();
    /// let ptr = SharedTrc::into_raw(shared2);
    ///
    /// assert_eq!(unsafe { *ptr }, 100);
    ///
    /// unsafe { SharedTrc::increment_local_count(ptr) };
    ///
    /// assert_eq!(SharedTrc::atomic_count(&shared), 3);
    ///
    /// unsafe { SharedTrc::decrement_local_count(ptr) };
    /// unsafe { SharedTrc::from_raw(ptr) };
    /// ```
    ///
    pub unsafe fn increment_local_count(ptr: *const T) {
        let trc = ManuallyDrop::new(SharedTrc::from_raw(ptr));
        let _: ManuallyDrop<_> = trc.clone();
    }
}

impl<T: ?Sized> Deref for SharedTrc<T> {
    type Target = T;

    /// Get an immutable reference to the internal data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    /// use std::ops::Deref;
    ///
    /// let mut shared: SharedTrc<_> = Trc::new(100i32).into();
    /// assert_eq!(*shared, 100i32);
    /// assert_eq!(shared.deref(), &100i32);
    /// ```
    #[inline]
    fn deref(&self) -> &Self::Target {
        &unsafe { self.data.as_ref() }.data
    }
}

#[inline(always)]
fn sum_value(value: &AtomicUsize, offset: usize, ordering: core::sync::atomic::Ordering) -> usize {
    #[cfg(immortals)]
    if value.load(core::sync::atomic::Ordering::Acquire) != usize::MAX {
        value.fetch_add(offset, ordering)
    } else {
        usize::MAX
    }

    #[cfg(not(immortals))]
    value.fetch_add(offset, ordering)
}

#[inline(always)]
fn sub_value(value: &AtomicUsize, offset: usize, ordering: core::sync::atomic::Ordering) -> usize {
    #[cfg(immortals)]
    if value.load(core::sync::atomic::Ordering::Acquire) != usize::MAX {
        value.fetch_sub(offset, ordering)
    } else {
        usize::MAX
    }

    #[cfg(not(immortals))]
    value.fetch_sub(offset, ordering)
}

impl<T> Trc<T> {
    /// Creates a new `Trc` from the provided data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// assert_eq!(*trc, 100);
    /// ```
    #[inline]
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

    /// Creates a new uninitialized `Trc`.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let mut trc = Trc::new_uninit();
    ///
    /// Trc::get_mut(&mut trc).unwrap().write(5);
    ///
    /// let five = unsafe { trc.assume_init() };
    ///
    /// assert_eq!(*five, 5);
    /// ```
    #[inline]
    pub fn new_uninit() -> Trc<MaybeUninit<T>> {
        let shareddata = SharedTrcInternal {
            atomicref: AtomicUsize::new(1),
            weakcount: AtomicUsize::new(1),
            data: MaybeUninit::<T>::uninit(),
        };

        let sbx = Box::new(shareddata);

        let tbx = Box::new(1);

        Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: NonNull::from(Box::leak(sbx)),
        }
    }

    /// Creates a new cyclic `Trc` from the provided data. It allows the storage of `Weak` which points the the allocation
    /// of `Trc`inside of `T`. Holding a `Trc` inside of `T` would cause a memory leak. This method works around this by
    /// providing a `Weak` during the construction of the `Trc`, so that the `T` can store the `Weak` internally.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// struct T(Weak<T>);
    ///
    /// let trc = Trc::new_cyclic(|x| T(x.clone()));
    /// ```
    #[inline]
    pub fn new_cyclic<F>(data_fn: F) -> Self
    where
        F: FnOnce(&Weak<T>) -> T,
    {
        let shareddata: NonNull<_> = Box::leak(Box::new(SharedTrcInternal {
            atomicref: AtomicUsize::new(0),
            weakcount: AtomicUsize::new(1),
            data: core::mem::MaybeUninit::<T>::uninit(),
        }))
        .into();

        let init_ptr: NonNull<SharedTrcInternal<T>> = shareddata.cast();

        let weak: Weak<T> = Weak { data: init_ptr };
        let data = data_fn(&weak);
        core::mem::forget(weak);

        unsafe {
            let ptr = init_ptr.as_ptr();
            core::ptr::write(core::ptr::addr_of_mut!((*ptr).data), data);

            let prev = sum_value(
                &init_ptr.as_ref().atomicref,
                1,
                core::sync::atomic::Ordering::AcqRel,
            );
            if prev > MAX_REFCOUNT {
                panic!("Overflow of maximum atomic reference count.");
            }
        }

        let tbx = Box::new(1);

        Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: init_ptr,
        }
    }

    /// Creates a new pinned `Trc`. If `T` does not implement [`Unpin`], then the data will be pinned in memory and unable to be moved.
    #[inline]
    pub fn pin(data: T) -> Pin<Trc<T>> {
        unsafe { Pin::new_unchecked(Trc::new(data)) }
    }

    /// Returns the inner value if the `Trc` has exactly one atomic and local reference.
    /// Otherwise, an [`Err`] is returned with the same `Trc` that was passed in.
    /// This will succeed even if there are outstanding weak references.
    ///
    /// # Examples
    /// This works:
    /// ```
    /// use trc::Trc;
    /// use std::ops::DerefMut;
    ///
    /// let mut trc = Trc::new(100);
    /// let inner = Trc::try_unwrap(trc).ok();
    /// ```
    ///
    /// This does not work:
    /// ```
    /// use trc::Trc;
    /// use std::ops::DerefMut;
    ///
    /// let mut trc = Trc::new(100);
    /// let _ = trc.clone();
    /// let inner = Trc::try_unwrap(trc).ok();
    /// ```
    #[inline]
    pub fn try_unwrap(mut this: Self) -> Result<T, Self> {
        if unsafe { this.shared.as_ref() }
            .atomicref
            .load(core::sync::atomic::Ordering::Acquire)
            != 1
            || *unsafe { this.threadref.as_ref() } != 1
        {
            return Err(this);
        }
        *unsafe { this.threadref.as_mut() } -= 1;

        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);

        unsafe {
            let elem = ptr::read(&this.shared.as_ref().data);
            drop(Box::from_raw(this.threadref.as_ptr()));

            //Clean up implicit self-reference
            drop(Weak { data: this.shared });
            core::mem::forget(this);

            Ok(elem)
        }
    }

    /// Returns the inner value if the `Trc` has exactly one atomic and local reference.
    /// Otherwise, a [`None`] is returned and the `Trc` is dropped.
    /// This will succeed even if there are outstanding weak references.
    /// If `into_inner` is called on every clone of `Trc`, it is guaranteed that exactly one will return the inner value `T`.
    /// This means the inner value is not dropped. The similar expression `Trc::try_unwrap(this).ok` does not offer such a guarantee.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let x = Trc::new(3i32);
    /// let y = Trc::clone(&x);
    ///
    /// let shared_x: SharedTrc<i32> = x.into();
    /// let shared_y: SharedTrc<i32> = y.into();
    ///
    /// // Two threads calling `Trc::into_inner` on both clones of an `Trc`:
    /// let x_thread = std::thread::spawn(|| Trc::into_inner(SharedTrc::to_trc(shared_x)));
    /// let y_thread = std::thread::spawn(|| Trc::into_inner(SharedTrc::to_trc(shared_y)));
    ///
    /// let x_inner_value = x_thread.join().unwrap();
    /// let y_inner_value = y_thread.join().unwrap();
    ///
    /// // One of the threads is guaranteed to receive the inner value:
    /// assert!(matches!(
    ///     (x_inner_value, y_inner_value),
    ///     (None, Some(3)) | (Some(3), None)
    /// ));
    /// ```
    ///
    #[inline]
    pub fn into_inner(this: Self) -> Option<T> {
        let this = core::mem::ManuallyDrop::new(this);

        if sub_value(
            &unsafe { this.shared.as_ref() }.atomicref,
            1,
            core::sync::atomic::Ordering::Release,
        ) != 1
            || *unsafe { this.threadref.as_ref() } != 1
        {
            drop(unsafe { Box::from_raw(this.threadref.as_ptr()) });
            return None;
        }

        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);

        let elem = unsafe { core::ptr::read(addr_of_mut!((*this.shared.as_ptr()).data)) };
        drop(unsafe { Box::from_raw(this.threadref.as_ptr()) });

        //Clean up implicit self-reference
        drop(Weak { data: this.shared });

        Some(elem)
    }

    pub fn create_immortal(self) -> Self {
        unsafe { self.shared.as_ref() }
            .atomicref
            .store(usize::MAX, core::sync::atomic::Ordering::SeqCst);
        unsafe { self.shared.as_ref() }
            .weakcount
            .store(usize::MAX, core::sync::atomic::Ordering::SeqCst);
        self
    }
}

impl<T> Trc<[T]> {
    /// Constructs a new `Trc` slice with uninitialized contents.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let mut trc = Trc::new_uninit();
    ///
    /// Trc::get_mut(&mut trc).unwrap().write(5);
    ///
    /// let five = unsafe { trc.assume_init() };
    ///
    /// assert_eq!(*five, 5);
    /// ```
    pub fn new_uninit_slice(len: usize) -> Trc<[MaybeUninit<T>]> {
        let value_layout = Layout::array::<T>(len).unwrap();
        let layout = Layout::new::<SharedTrcInternal<()>>()
            .extend(value_layout)
            .unwrap()
            .0
            .pad_to_align();

        let res = slice_from_raw_parts_mut(unsafe { alloc(layout) } as *mut T, len)
            as *mut SharedTrcInternal<[MaybeUninit<T>]>;
        unsafe { write(&mut (*res).atomicref, AtomicUsize::new(1)) };
        unsafe { write(&mut (*res).weakcount, AtomicUsize::new(1)) };

        let elems = unsafe { addr_of_mut!((*res).data) } as *mut MaybeUninit<T>;
        for i in 0..len {
            unsafe { write(elems.add(i), MaybeUninit::<T>::uninit()) };
        }
        let tbx = Box::new(1);

        Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: unsafe { NonNull::new_unchecked(res) },
        }
    }
}

impl<T> Trc<MaybeUninit<T>> {
    /// Assume that `Trc<MaybeUninit<T>>` is initialized, converting it to `Trc<T>`.
    ///
    /// # Safety
    /// As with `MaybeUninit::assume_init`, it is up to the caller to guarantee that the inner value really is in an initialized state.
    /// Calling this when the content is not yet fully initialized causes immediate undefined behavior.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let mut values = Trc::<[u32]>::new_uninit_slice(3);
    ///
    /// // Deferred initialization:
    /// let data = Trc::get_mut(&mut values).unwrap();
    /// data[0].write(1);
    /// data[1].write(2);
    /// data[2].write(3);
    ///
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [1, 2, 3])
    /// ```
    pub unsafe fn assume_init(self) -> Trc<T> {
        let threadref = self.threadref;
        Trc {
            shared: NonNull::new_unchecked(ManuallyDrop::new(self).shared.as_ptr().cast()),
            threadref,
        }
    }
}

impl<T> Trc<[MaybeUninit<T>]> {
    /// Assume that all elements in `Trc<[MaybeUninit<T>]>` are initialized, converting it to `Trc<[T]>`.
    ///
    /// # Safety
    /// As with `MaybeUninit::assume_init`, it is up to the caller to guarantee that the inner value really is in an initialized state.
    /// Calling this when the content is not yet fully initialized causes immediate undefined behavior.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let mut values = Trc::<[u32]>::new_uninit_slice(3);
    ///
    /// // Deferred initialization:
    /// let data = Trc::get_mut(&mut values).unwrap();
    /// data[0].write(1);
    /// data[1].write(2);
    /// data[2].write(3);
    ///
    /// let values = unsafe { values.assume_init() };
    ///
    /// assert_eq!(*values, [1, 2, 3])
    /// ```
    pub unsafe fn assume_init(self) -> Trc<[T]> {
        let threadref = self.threadref;
        Trc {
            shared: NonNull::new_unchecked(ManuallyDrop::new(self).shared.as_ptr() as _),
            threadref,
        }
    }
}

impl<T: ?Sized> Trc<T> {
    /// Return the local thread reference count of the object, which is how many `Trc`s in this thread point to the data referenced by this `Trc`.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// assert!(Trc::local_count(&trc) == 1)
    /// ```
    #[inline]
    pub fn local_count(this: &Self) -> usize {
        *unsafe { this.threadref.as_ref() }
    }

    /// Return the atomic reference count of the object. This is how many threads are using the data referenced by this `Trc`.
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
    ///     assert_eq!(Trc::atomic_count(&trc), 2);
    /// });
    ///
    /// handle.join().unwrap();
    /// assert_eq!(Trc::atomic_count(&trc), 1);
    /// assert_eq!(*trc, 100);
    /// ```
    #[inline]
    pub fn atomic_count(this: &Self) -> usize {
        unsafe { this.shared.as_ref() }
            .atomicref
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    /// Return the weak count of the object. This is how many weak counts - across all threads - are pointing to the allocation inside of `Trc`.
    /// It includes the implicit weak reference held by all `Trc` or `SharedTrc` to themselves.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100i32);
    /// let weak = Trc::downgrade(&trc);
    /// let weak2 = Trc::downgrade(&trc);
    /// let new_trc = Weak::upgrade(&weak).expect("Value was dropped");
    /// drop(weak);
    /// assert_eq!(Trc::weak_count(&new_trc), 2);
    /// ```
    #[inline]
    pub fn weak_count(this: &Self) -> usize {
        unsafe { this.shared.as_ref() }
            .weakcount
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    /// Checks if the other `Trc` is equal to this one according to their internal pointers.
    ///
    /// # Examples
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

    /// Gets the raw pointer to the most inner layer of `Trc`. This is only valid if there are at least some atomic references.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// println!("{}", Trc::as_ptr(&trc) as usize)
    /// ```
    #[inline]
    pub fn as_ptr(this: &Self) -> *const T {
        let sharedptr = NonNull::as_ptr(this.shared);
        unsafe { addr_of_mut!((*sharedptr).data) }
    }

    /// Get a &mut reference to the internal data if there are no other `Trc` or [`Weak`] pointers to the same allocation.
    /// Otherwise, return [`None`] because it would be unsafe to mutate a shared value.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use std::ops::DerefMut;
    ///
    /// let mut trc = Trc::new(100);
    /// let mutref = Trc::get_mut(&mut trc).unwrap();
    /// *mutref = 300;
    /// assert_eq!(*trc, 300);
    /// ```
    #[inline]
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        //Acquire the weakcount if it is == 1
        if unsafe { this.shared.as_ref() }
            .weakcount
            .compare_exchange(
                1,
                usize::MAX,
                core::sync::atomic::Ordering::Acquire,
                core::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
        {
            //Acquire the atomicref
            let unique = unsafe { this.shared.as_ref() }
                .atomicref
                .load(core::sync::atomic::Ordering::Acquire)
                == 1;

            //Synchronize with the previous Acquire
            unsafe { this.shared.as_ref() }
                .weakcount
                .store(1, core::sync::atomic::Ordering::Release);

            if unique && *unsafe { this.threadref.as_ref() } == 1 {
                Some(unsafe { &mut (*this.shared.as_ptr()).data })
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl<T: Clone> Trc<T> {
    /// If we have the only strong and local reference to `T`, then unwrap it. Otherwise, clone `T` and return the clone.
    /// If `trc_t` is of type `Trc<T>`, this function is functionally equivalent to `(*trc_t).clone()`, but will avoid cloning the inner
    /// value where possible.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let inner = String::from("Trc");
    /// let ptr = inner.as_ptr();
    ///
    /// let trc = Trc::new(inner);
    /// let trc2 = trc.clone();
    /// let inner = Trc::unwrap_or_clone(trc);
    /// assert!(!std::ptr::eq(ptr, inner.as_ptr()));
    ///
    /// let inner = Trc::unwrap_or_clone(trc2);
    /// assert!(std::ptr::eq(ptr, inner.as_ptr()));
    /// ```
    #[inline]
    pub fn unwrap_or_clone(this: Self) -> T {
        Trc::try_unwrap(this).unwrap_or_else(|trc| (*trc).clone())
    }
}

#[cfg(feature = "dyn_unstable")]
impl Trc<dyn Any + Send + Sync> {
    /// Attempts to downcast a `Trc<dyn Any + Send + Sync>` into a concrete type.
    ///
    /// # Examples
    /// ```
    /// use std::any::Any;
    /// use trc::Trc;
    ///
    /// fn print_if_string(value: Trc<dyn Any + Send + Sync>) {
    ///     if let Ok(string) = value.downcast::<String>() {
    ///         println!("String ({}): {}", string.len(), string);
    ///     }
    /// }
    ///
    /// let my_string = "Hello World".to_string();
    /// print_if_string(Trc::new(my_string));
    /// print_if_string(Trc::new(0i8));
    /// ```
    pub fn downcast<T>(self) -> Result<Trc<T>, Self>
    where
        T: Any + Send + Sync,
    {
        if (*self).is::<T>() {
            let shared = self.shared.cast::<SharedTrcInternal<T>>();
            let threadref = self.threadref;
            forget(self);
            Ok(Trc { shared, threadref })
        } else {
            Err(self)
        }
    }
}

impl<T: ?Sized> Trc<T> {
    /// Downgrade a `Trc` to a `Weak`. This increments the weak count.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let weak = Trc::downgrade(&trc);
    /// ```
    #[inline]
    pub fn downgrade(trc: &Trc<T>) -> Weak<T> {
        let prev = sum_value(
            &unsafe { trc.shared.as_ref() }.weakcount,
            1,
            core::sync::atomic::Ordering::Acquire,
        );
        if prev > MAX_REFCOUNT {
            panic!("Overflow of maximum weak reference count.");
        }
        Weak { data: trc.shared }
    }
}

impl<T: ?Sized> Deref for Trc<T> {
    type Target = T;

    /// Get an immutable reference to the internal data.
    ///
    /// # Examples
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

impl<T: ?Sized> Drop for Trc<T> {
    #[cfg(immortals)]
    #[inline]
    fn drop(&mut self) {
        if unsafe { self.shared.as_ref() }
            .atomicref
            .load(core::sync::atomic::Ordering::Acquire)
            != usize::MAX
        {
            //If it is not immortal
            *unsafe { self.threadref.as_mut() } -= 1;
            if *unsafe { self.threadref.as_ref() } == 0 {
                drop(unsafe { Box::from_raw(self.threadref.as_ptr()) });
                if sub_value(
                    &unsafe { self.shared.as_ref() }.atomicref,
                    1,
                    core::sync::atomic::Ordering::Release,
                ) != 1
                {
                    return;
                }

                core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
                unsafe { core::ptr::drop_in_place(addr_of_mut!((*self.shared.as_ptr()).data)) };
                Weak { data: self.shared };
            }
        }
    }

    #[cfg(not(immortals))]
    #[inline]
    fn drop(&mut self) {
        *unsafe { self.threadref.as_mut() } -= 1;
        if *unsafe { self.threadref.as_ref() } == 0 {
            drop(unsafe { Box::from_raw(self.threadref.as_ptr()) });
            if sub_value(
                &unsafe { self.shared.as_ref() }.atomicref,
                1,
                core::sync::atomic::Ordering::Release,
            ) != 1
            {
                return;
            }

            core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
            unsafe { core::ptr::drop_in_place(addr_of_mut!((*self.shared.as_ptr()).data)) };
            Weak { data: self.shared };
        }
    }
}

impl<T: ?Sized> Clone for Trc<T> {
    /// Clone a `Trc` (increment it's local reference count).
    /// It will panic if the local reference count overflows.
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::new(100);
    /// let trc2 = trc.clone();
    /// assert_eq!(Trc::local_count(&trc), Trc::local_count(&trc2));
    /// ```
    #[inline(always)]
    fn clone(&self) -> Self {
        unsafe { *self.threadref.as_ptr() += 1 };
        if unsafe { *self.threadref.as_ptr() } > MAX_REFCOUNT {
            panic!("Overflow of maximum atomic reference count.");
        }

        Trc {
            shared: self.shared,
            threadref: self.threadref,
        }
    }
}

impl<T: ?Sized> AsRef<T> for Trc<T> {
    fn as_ref(&self) -> &T {
        Trc::deref(self)
    }
}

impl<T: ?Sized> AsRef<T> for SharedTrc<T> {
    fn as_ref(&self) -> &T {
        SharedTrc::deref(self)
    }
}

impl<T: ?Sized> Borrow<T> for Trc<T> {
    fn borrow(&self) -> &T {
        self.as_ref()
    }
}

impl<T: ?Sized> Borrow<T> for SharedTrc<T> {
    fn borrow(&self) -> &T {
        self.as_ref()
    }
}

impl<T: ?Sized + Default> Default for Trc<T> {
    fn default() -> Self {
        Trc::new(Default::default())
    }
}

impl<T: ?Sized + Default> Default for SharedTrc<T> {
    fn default() -> Self {
        Self::from_trc(&Trc::new(Default::default()))
    }
}

impl<T: Display> Display for Trc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt((*self).deref(), f)
    }
}

impl<T: Display> Display for SharedTrc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt((*self).deref(), f)
    }
}

impl<T: Debug> Debug for Trc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt((*self).deref(), f)
    }
}

impl<T: Debug> Debug for SharedTrc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt((*self).deref(), f)
    }
}

impl<T: ?Sized> Pointer for Trc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Pointer::fmt(&addr_of!(unsafe { self.shared.as_ref() }.data), f)
    }
}

impl<T: ?Sized> Pointer for SharedTrc<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Pointer::fmt(&addr_of!(unsafe { self.data.as_ref() }.data), f)
    }
}

impl<T> From<T> for Trc<T> {
    /// Create a new `Trc` from the provided data. This is equivalent to calling `Trc::new` on the same data.
    ///
    /// # Examples
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
    /// Pass the data contained in this `Trc` to the provided hasher.
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl<T: Hash> Hash for SharedTrc<T> {
    /// Pass the data contained in this `SharedTrc` to the provided hasher.
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.deref().hash(state);
    }
}

impl<T: PartialOrd> PartialOrd for Trc<T> {
    /// "Greater than or equal to" comparison for two `Trc`s.
    ///
    /// Calls `.ge` on the data.
    ///
    /// # Examples
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

    /// "Less than or equal to" comparison for two `Trc`s.
    ///
    /// Calls `.le` on the data.
    ///
    /// # Examples
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

    /// "Greater than" comparison for two `Trc`s.
    ///
    /// Calls `.gt` on the data.
    ///
    /// # Examples
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

    /// "Less than" comparison for two `Trc`s.
    ///
    /// Calls `.lt` on the data.
    ///
    /// # Examples
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

    /// Partial comparison for two `Trc`s.
    ///
    /// Calls `.partial_cmp` on the data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use std::cmp::Ordering;
    ///
    /// let trc1 = Trc::from(100);
    /// let trc2 = Trc::from(200);
    /// assert_eq!(Some(Ordering::Less), trc1.partial_cmp(&trc2));
    /// ```
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<T: PartialOrd> PartialOrd for SharedTrc<T> {
    /// "Greater than or equal to" comparison for two `SharedTrc`s.
    ///
    /// Calls `.ge` on the data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared1: SharedTrc<_> = Trc::from(100).into();
    /// let shared2 = shared1.clone();
    /// assert!(shared1>=shared2);
    /// ```
    #[inline]
    fn ge(&self, other: &Self) -> bool {
        self.deref().ge(other.deref())
    }

    /// "Less than or equal to" comparison for two `SharedTrc`s.
    ///
    /// Calls `.le` on the data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared1: SharedTrc<_> = Trc::from(100).into();
    /// let shared2 = shared1.clone();
    /// assert!(shared1<=shared2);
    /// ```
    #[inline]
    fn le(&self, other: &Self) -> bool {
        self.deref().ge(other.deref())
    }

    /// "Greater than" comparison for two `SharedTrc`s.
    ///
    /// Calls `.gt` on the data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared1: SharedTrc<_> = Trc::from(200).into();
    /// let shared2: SharedTrc<_> = Trc::from(100).into();
    /// assert!(shared1>shared2);
    /// ```
    #[inline]
    fn gt(&self, other: &Self) -> bool {
        self.deref().gt(other.deref())
    }

    /// "Less than" comparison for two `SharedTrc`s.
    ///
    /// Calls `.lt` on the data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared1: SharedTrc<_> = Trc::from(100).into();
    /// let shared2: SharedTrc<_> = Trc::from(200).into();
    /// assert!(shared1<shared2);
    /// ```
    #[inline]
    fn lt(&self, other: &Self) -> bool {
        self.deref().lt(other.deref())
    }

    /// Partial comparison for two `SharedTrc`s.
    ///
    /// Calls `.partial_cmp` on the data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    /// use std::cmp::Ordering;
    ///
    /// let shared1: SharedTrc<_> = Trc::from(100).into();
    /// let shared2: SharedTrc<_> = Trc::from(200).into();
    /// assert_eq!(Some(Ordering::Less), shared1.partial_cmp(&shared2));
    /// ```
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<T: Ord> Ord for Trc<T> {
    /// Comparison for two `Trc`s. The two are compared by calling `.cmp` on the inner values.
    ///
    /// # Examples
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

impl<T: Ord> Ord for SharedTrc<T> {
    /// Comparison for two `SharedTrc`s. The two are compared by calling `.cmp` on the inner values.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    /// use std::cmp::Ordering;
    ///
    /// let shared1: SharedTrc<_> = Trc::from(100).into();
    /// let shared2: SharedTrc<_> = Trc::from(200).into();
    /// assert_eq!(Ordering::Less, shared1.cmp(&shared2));
    /// ```
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.deref().cmp(other.deref())
    }
}

impl<T: Eq> Eq for Trc<T> {}

impl<T: Eq> Eq for SharedTrc<T> {}

impl<T: PartialEq> PartialEq for Trc<T> {
    /// Equality by value comparison for two `Trc`s, even if the data is in different allocoations.
    ///
    /// Calls `.eq` on the data.
    ///
    /// # Examples
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

    /// Inequality by value comparison for two `Trc`s, even if the data is in different allocoations.
    ///
    /// Calls `.ne` on the data.
    ///
    /// # Examples
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

impl<T: PartialEq> PartialEq for SharedTrc<T> {
    /// Equality by value comparison for two `SharedTrc`s, even if the data is in different allocoations.
    ///
    /// Calls `.eq` on the data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared1: SharedTrc<_> = Trc::from(100).into();
    /// let shared2: SharedTrc<_> = Trc::from(100).into();
    /// assert!(shared1==shared2);
    /// ```
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }

    /// Inequality by value comparison for two `SharedTrc`s, even if the data is in different allocoations.
    ///
    /// Calls `.ne` on the data.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    ///
    /// let shared1: SharedTrc<_> = Trc::from(100).into();
    /// let shared2: SharedTrc<_> = Trc::from(200).into();
    /// assert!(shared1!=shared2);
    /// ```
    #[allow(clippy::partialeq_ne_impl)]
    #[inline]
    fn ne(&self, other: &Self) -> bool {
        self.deref().ne(other.deref())
    }
}

#[cfg(not(target_os = "windows"))]
impl<T: AsFd> AsFd for Trc<T> {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        (**self).as_fd()
    }
}

#[cfg(not(target_os = "windows"))]
impl<T: AsFd> AsFd for SharedTrc<T> {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        (**self).as_fd()
    }
}

#[cfg(target_os = "windows")]
impl<T: AsRawHandle> AsRawHandle for Trc<T> {
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        (**self).as_raw_handle()
    }
}

#[cfg(target_os = "windows")]
impl<T: AsRawHandle> AsRawHandle for SharedTrc<T> {
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        (**self).as_raw_handle()
    }
}

#[cfg(target_os = "windows")]
impl<T: AsHandle> AsHandle for Trc<T> {
    fn as_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        (**self).as_handle()
    }
}

#[cfg(target_os = "windows")]
impl<T: AsHandle> AsHandle for SharedTrc<T> {
    fn as_handle(&self) -> std::os::windows::io::BorrowedHandle<'_> {
        (**self).as_handle()
    }
}

#[cfg(not(target_os = "windows"))]
impl<T: AsRawFd> AsRawFd for Trc<T> {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        (**self).as_raw_fd()
    }
}

#[cfg(not(target_os = "windows"))]
impl<T: AsRawFd> AsRawFd for SharedTrc<T> {
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        (**self).as_raw_fd()
    }
}

#[cfg(target_os = "windows")]
impl<T: AsRawSocket> AsRawSocket for Trc<T> {
    fn as_raw_socket(&self) -> std::os::windows::io::RawSocket {
        (**self).as_raw_socket()
    }
}

#[cfg(target_os = "windows")]
impl<T: AsRawSocket> AsRawSocket for SharedTrc<T> {
    fn as_raw_socket(&self) -> std::os::windows::io::RawSocket {
        (**self).as_raw_socket()
    }
}

#[cfg(target_os = "windows")]
impl<T: AsSocket> AsSocket for Trc<T> {
    fn as_socket(&self) -> std::os::windows::io::BorrowedSocket<'_> {
        (**self).as_socket()
    }
}

#[cfg(target_os = "windows")]
impl<T: AsSocket> AsSocket for SharedTrc<T> {
    fn as_socket(&self) -> std::os::windows::io::BorrowedSocket<'_> {
        (**self).as_socket()
    }
}

#[allow(deprecated)]
impl<T: Error> Error for Trc<T> {
    fn cause(&self) -> Option<&dyn Error> {
        (**self).cause()
    }
    fn description(&self) -> &str {
        (**self).description()
    }
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        (**self).source()
    }
}

#[allow(deprecated)]
impl<T: Error> Error for SharedTrc<T> {
    fn cause(&self) -> Option<&dyn Error> {
        (**self).cause()
    }
    fn description(&self) -> &str {
        (**self).description()
    }
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        (**self).source()
    }
}

impl<T: ?Sized> Unpin for Trc<T> {}
impl<T: ?Sized> UnwindSafe for Trc<T> {}

impl<T: ?Sized> Unpin for SharedTrc<T> {}
impl<T: ?Sized> UnwindSafe for SharedTrc<T> {}

unsafe impl<T: Sync + Send> Send for SharedTrc<T> {}
unsafe impl<T: Sync + Send> Sync for SharedTrc<T> {}

unsafe impl<T: Sync + Send> Send for Weak<T> {}
unsafe impl<T: Sync + Send> Sync for Weak<T> {}

fn create_from_iterator_exact<T>(
    iterator: impl Iterator<Item = T> + ExactSizeIterator,
) -> *mut SharedTrcInternal<[T]> {
    let value_layout = Layout::array::<T>(iterator.len()).unwrap();
    let layout = Layout::new::<SharedTrcInternal<()>>()
        .extend(value_layout)
        .unwrap()
        .0
        .pad_to_align();

    let res = slice_from_raw_parts_mut(unsafe { alloc(layout) } as *mut T, iterator.len())
        as *mut SharedTrcInternal<[T]>;
    unsafe { write(&mut (*res).atomicref, AtomicUsize::new(1)) };
    unsafe { write(&mut (*res).weakcount, AtomicUsize::new(1)) };

    let elems = unsafe { addr_of_mut!((*res).data) } as *mut T;
    for (n, i) in iterator.enumerate() {
        unsafe { write(elems.add(n), i) };
    }
    res
}

trait TrcFromIter<T> {
    fn from_iter(slice: impl Iterator<Item = T> + ExactSizeIterator) -> Self;
}

impl<T: Clone + ?Sized> TrcFromIter<T> for Trc<[T]> {
    fn from_iter(slice: impl Iterator<Item = T> + ExactSizeIterator) -> Self {
        let shared = create_from_iterator_exact(slice);
        let tbx = Box::new(1);

        Trc {
            threadref: NonNull::from(Box::leak(tbx)),
            shared: unsafe { NonNull::new_unchecked(shared) },
        }
    }
}

impl<T: Clone + ?Sized> From<&[T]> for Trc<[T]> {
    /// From conversion from a reference to a slice of type `T` (`&[T]`) to a `Trc<[T]>`.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let vec = (1..100).collect::<Vec<i32>>();
    /// let slice = &vec[2..5];
    /// let trc = Trc::<[i32]>::from(slice);
    /// assert_eq!(&*trc, slice);
    /// ```
    fn from(value: &[T]) -> Trc<[T]> {
        <Self as TrcFromIter<T>>::from_iter(value.iter().cloned())
    }
}

impl<T: Clone + ?Sized> FromIterator<T> for Trc<[T]> {
    /// From conversion from an iterator (`impl IntoIterator<Item = T>`) to `Trc<[T]>`. Due to Rust's unstable trait specialization feature,
    /// there is no special case for iterators that implement [`ExactSizeIterator`].
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    ///
    /// let trc = Trc::<[i32]>::from_iter(vec![1,2,3]);
    /// assert_eq!(&*trc, vec![1,2,3]);
    /// ```
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self::from(&iter.into_iter().collect::<Vec<_>>()[..])
    }
}

//TODO: Integration with standard library for both, or use lib & conditional for just CoerceUnsized
#[cfg(feature = "dyn_unstable")]
impl<T: ?Sized + std::marker::Unsize<U>, U: ?Sized> std::ops::CoerceUnsized<Trc<U>> for Trc<T> {}

#[cfg(feature = "dyn_unstable")]
impl<T: ?Sized> std::ops::Receiver for Trc<T> {}
//Because Trc is !DispatchFromDyn, fn _(self: Trc<Self>) cannot be implemented.

#[cfg(feature = "dyn_unstable")]
impl<T: ?Sized + std::marker::Unsize<U>, U: ?Sized> std::ops::CoerceUnsized<SharedTrc<U>>
    for SharedTrc<T>
{
}

#[cfg(feature = "dyn_unstable")]
impl<T: ?Sized> std::ops::Receiver for SharedTrc<T> {}

#[cfg(feature = "dyn_unstable")]
impl<T: ?Sized, U: ?Sized> core::ops::DispatchFromDyn<SharedTrc<U>> for SharedTrc<T> where
    T: std::marker::Unsize<U>
{
}
//Because SharedTrc is !DispatchFromDyn, fn _(self: SharedTrc<Self>) cannot be implemented.

impl<T: ?Sized> Drop for Weak<T> {
    #[inline]
    fn drop(&mut self) {
        if sub_value(
            unsafe { &(*self.data.as_ptr()).weakcount },
            1,
            core::sync::atomic::Ordering::Release,
        ) != 1
        {
            return;
        }

        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);

        let layout = Layout::for_value(unsafe { &*self.data.as_ptr() });
        unsafe { std::alloc::dealloc(self.data.as_ptr().cast(), layout) };
    }
}

impl<T: ?Sized> Weak<T> {
    /// Upgrade a `Weak` to a `Trc`. Because `Weak` does not own the value, it may have been dropped already. If it has, a `None` is returned.
    /// If the value has not been dropped, then this function increments the atomic reference count of the object.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100i32);
    /// let weak = Trc::downgrade(&trc);
    /// let new_trc = weak.upgrade().expect("Value was dropped");
    /// assert_eq!(*new_trc, 100i32);
    /// ```
    #[inline]
    pub fn upgrade(&self) -> Option<Trc<T>> {
        unsafe { self.data.as_ref() }
            .atomicref
            .fetch_update(
                core::sync::atomic::Ordering::Acquire,
                core::sync::atomic::Ordering::Relaxed,
                |n| {
                    // Any write of 0 we can observe leaves the field in permanently zero state.
                    if n == 0 {
                        return None;
                    }
                    // See comments in `Trc::clone` for why we do this (for `mem::forget`).
                    assert!(
                        n <= MAX_REFCOUNT,
                        "Overflow of maximum atomic reference count."
                    );
                    Some(n + 1)
                },
            )
            .ok()
            .map(|_| {
                let tbx = Box::new(1);
                Trc {
                    threadref: NonNull::from(Box::leak(tbx)),
                    shared: self.data,
                }
            })
    }

    /// Gets the raw pointer to the most inner layer of `Weak`. The data is only valid (not dropped) if there are at least some atomic references.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let weak = Trc::downgrade(&trc);
    /// println!("{}", Trc::as_ptr(&trc) as usize)
    /// ```
    #[inline]
    pub fn as_ptr(this: &Self) -> *const T {
        let sharedptr = NonNull::as_ptr(this.data);
        unsafe { addr_of_mut!((*sharedptr).data) }
    }

    /// Converts a `Weak` into `*const T`, without freeing the allocation.
    /// To avoid a memory leak, be sure to call [`Weak::from_raw`] to reclaim the allocation.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let weak = Trc::downgrade(&trc);
    /// let ptr = Weak::into_raw(weak);
    ///
    /// assert_eq!(unsafe { *ptr }, 100);
    ///
    /// unsafe { Weak::from_raw(ptr) };
    /// ```
    pub fn into_raw(this: Self) -> *const T {
        let ptr = Self::as_ptr(&this);

        forget(this);
        ptr
    }
}

impl<T> Weak<T> {
    /// Converts a `*const T` into `Weak`. The caller must uphold the below safety constraints.
    ///
    /// # Safety
    /// - The given pointer must be a valid pointer to `T` that came from [`Weak::into_raw`].
    /// - After `from_raw`, the pointer must not be accessed.
    ///
    /// # Examples
    /// Example 1:
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let weak = Trc::downgrade(&Trc::new(100));
    /// let ptr = Weak::into_raw(weak);
    ///
    /// assert_eq!(unsafe { *ptr }, 100);
    ///
    /// unsafe { Weak::from_raw(ptr) };
    /// ```
    /// Example 2:
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let strong = Trc::new("hello".to_owned());
    ///
    /// let raw_1 = Weak::into_raw(Trc::downgrade(&strong));
    /// let raw_2 = Weak::into_raw(Trc::downgrade(&strong));
    ///
    /// assert_eq!(3, Trc::weak_count(&strong));
    ///
    /// assert_eq!("hello", &*Weak::upgrade(unsafe { &Weak::from_raw(raw_1) }).unwrap());
    /// assert_eq!(2, Trc::weak_count(&strong));
    ///
    /// drop(strong);
    ///
    /// // Decrement the last weak count.
    /// assert!( Weak::upgrade(unsafe {& Weak::from_raw(raw_2) }).is_none());
    /// ```
    pub unsafe fn from_raw(ptr: *const T) -> Self {
        let layout = Layout::new::<SharedTrcInternal<()>>();
        let n = layout.size();

        let data_ptr = (ptr as *const u8).sub(n) as *mut SharedTrcInternal<T>;

        Weak {
            data: NonNull::new_unchecked(data_ptr),
        }
    }

    /// Create a new, uninitialized `Weak`. Calling [`Weak::upgrade`] on this will always return `None.
    ///
    /// # Examples
    /// ```
    /// use trc::Weak;
    /// use core::mem::MaybeUninit;
    ///
    /// let weak: Weak<MaybeUninit<i32>> = Weak::new();
    ///
    /// assert!(Weak::upgrade(&weak).is_none());
    /// ```
    pub fn new() -> Weak<MaybeUninit<T>> {
        let data = MaybeUninit::<T>::uninit();

        let shareddata = SharedTrcInternal {
            atomicref: AtomicUsize::new(0),
            weakcount: AtomicUsize::new(1),
            data,
        };

        let sbx = Box::new(shareddata);

        Weak {
            data: NonNull::from(Box::leak(sbx)),
        }
    }

    /// Return the atomic reference count of the object. This is how many threads are using the data referenced by this `Weak`.
    ///
    /// # Examples
    /// ```
    /// use std::thread;
    /// use trc::Trc;
    /// use trc::SharedTrc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let shared = SharedTrc::from_trc(&trc);
    /// let weak = Trc::downgrade(&trc);
    ///
    /// let handle = thread::spawn(move || {
    ///     assert_eq!(Weak::atomic_count(&weak), 2);
    ///     let trc = SharedTrc::to_trc(shared);
    /// });
    ///
    /// handle.join().unwrap();
    /// assert_eq!(*trc, 100);
    /// ```
    #[inline]
    pub fn atomic_count(this: &Self) -> usize {
        unsafe { this.data.as_ref() }
            .atomicref
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    /// Return the weak count of the object. This is how many weak counts - across all threads - are pointing to the allocation inside of the `Weak`.
    /// It includes the implicit weak reference held by all `SharedTrc` or `Trc` to themselves.
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::SharedTrc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100i32);
    /// let weak = Trc::downgrade(&trc);
    /// let weak2 = Trc::downgrade(&trc);
    /// let new_trc = Weak::upgrade(&weak).expect("Value was dropped");
    /// drop(weak);
    /// let shared: SharedTrc<_> = trc.into();
    /// assert_eq!(SharedTrc::weak_count(&shared), 2);
    /// ```
    #[inline]
    pub fn weak_count(this: &Self) -> usize {
        unsafe { this.data.as_ref() }
            .weakcount
            .load(core::sync::atomic::Ordering::Relaxed)
    }
}

impl<T: ?Sized> Clone for Weak<T> {
    /// Clone a `Weak` (increment the weak count).
    ///
    /// # Examples
    /// ```
    /// use trc::Trc;
    /// use trc::Weak;
    ///
    /// let trc = Trc::new(100);
    /// let weak1 = Trc::downgrade(&trc);
    /// let weak2 = weak1.clone();
    /// assert_eq!(Trc::weak_count(&trc), 3);
    /// ```
    #[inline]
    fn clone(&self) -> Self {
        let prev = sum_value(
            &unsafe { self.data.as_ref() }.weakcount,
            1,
            core::sync::atomic::Ordering::Relaxed,
        );

        //If an absurd number of threads are created, and then they are aborted before this, UB can
        //occur if the refcount wraps around.
        if prev > MAX_REFCOUNT {
            panic!("Overflow of maximum weak reference count.");
        }

        Weak { data: self.data }
    }
}
