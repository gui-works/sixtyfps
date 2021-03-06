/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! module for the SharedArray and related things
#![allow(unsafe_code)]
#![warn(missing_docs)]
use core::fmt::Debug;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic;
use std::alloc;

#[repr(C)]
struct SharedArrayHeader {
    refcount: atomic::AtomicIsize,
    size: usize,
    capacity: usize,
}

#[repr(C)]
struct SharedArrayInner<T> {
    header: SharedArrayHeader,
    data: MaybeUninit<T>,
}

fn compute_inner_layout<T>(capacity: usize) -> alloc::Layout {
    alloc::Layout::new::<SharedArrayHeader>()
        .extend(alloc::Layout::array::<T>(capacity).unwrap())
        .unwrap()
        .0
}

unsafe fn drop_inner<T>(inner: NonNull<SharedArrayInner<T>>) {
    debug_assert_eq!(inner.as_ref().header.refcount.load(core::sync::atomic::Ordering::Relaxed), 0);
    let data_ptr = inner.as_ref().data.as_ptr();
    for x in 0..inner.as_ref().header.size {
        drop(core::ptr::read(data_ptr.add(x)));
    }
    alloc::dealloc(
        inner.as_ptr() as *mut u8,
        compute_inner_layout::<T>(inner.as_ref().header.capacity),
    )
}

/// Allocate the memory for the SharedArray with the given capacity. Return the inner with size and refcount set to 1
fn alloc_with_capacity<T>(capacity: usize) -> NonNull<SharedArrayInner<T>> {
    let ptr = unsafe { alloc::alloc(compute_inner_layout::<T>(capacity)) };
    unsafe {
        core::ptr::write(
            ptr as *mut SharedArrayHeader,
            SharedArrayHeader { refcount: 1.into(), size: 0, capacity },
        );
    }
    NonNull::new(ptr).unwrap().cast()
}

#[repr(C)]
/// SharedArray holds a reference-counted read-only copy of `[T]`.
pub struct SharedArray<T> {
    inner: NonNull<SharedArrayInner<T>>,
}

impl<T> Drop for SharedArray<T> {
    fn drop(&mut self) {
        unsafe {
            if self.inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed) < 0 {
                return;
            }
            if self.inner.as_ref().header.refcount.fetch_sub(1, atomic::Ordering::SeqCst) == 1 {
                drop_inner(self.inner)
            }
        }
    }
}

impl<T> Clone for SharedArray<T> {
    fn clone(&self) -> Self {
        unsafe {
            if self.inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed) > 0 {
                self.inner.as_ref().header.refcount.fetch_add(1, atomic::Ordering::SeqCst);
            }
            return SharedArray { inner: self.inner };
        }
    }
}

impl<T> SharedArray<T> {
    fn as_ptr(&self) -> *const T {
        unsafe { self.inner.as_ref().data.as_ptr() }
    }

    /// Size of the string, in bytes
    pub fn len(&self) -> usize {
        unsafe { self.inner.as_ref().header.size }
    }

    /// Return a slice to the array
    pub fn as_slice(&self) -> &[T] {
        unsafe { core::slice::from_raw_parts(self.as_ptr(), self.len()) }
    }

    /// Constructs a new SharedArray from the given iterator.
    pub fn from_iter(mut iter: impl Iterator<Item = T> + ExactSizeIterator) -> Self {
        let capacity = iter.len();
        let inner = alloc_with_capacity::<T>(capacity);
        let mut result = SharedArray { inner };
        let mut size = 0;
        while let Some(x) = iter.next() {
            assert_ne!(size, capacity);
            unsafe {
                core::ptr::write(result.inner.as_mut().data.as_mut_ptr().add(size), x);
                size += 1;
                result.inner.as_mut().header.size = size;
            }
        }
        result
    }

    /// Constructs a new SharedArray from the given slice.
    pub fn from(slice: &[T]) -> Self
    where
        T: Clone,
    {
        SharedArray::from_iter(slice.iter().cloned())
    }
}

impl<T> Deref for SharedArray<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

static SHARED_NULL: SharedArrayHeader =
    SharedArrayHeader { refcount: std::sync::atomic::AtomicIsize::new(-1), size: 0, capacity: 0 };

impl<T> Default for SharedArray<T> {
    fn default() -> Self {
        SharedArray { inner: NonNull::from(&SHARED_NULL).cast() }
    }
}

impl<T: Debug> Debug for SharedArray<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_slice().fmt(f)
    }
}

impl<T> AsRef<[T]> for SharedArray<T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, U> PartialEq<U> for SharedArray<T>
where
    U: ?Sized + AsRef<[T]>,
    T: PartialEq,
{
    fn eq(&self, other: &U) -> bool {
        self.as_slice() == other.as_ref()
    }
}

impl<T: Eq> Eq for SharedArray<T> {}

impl<T: Clone> IntoIterator for SharedArray<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(unsafe {
            if self.inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed) == 1 {
                let inner = self.inner;
                std::mem::forget(self);
                inner.as_ref().header.refcount.store(0, atomic::Ordering::Relaxed);
                IntoIterInner::UnShared(inner, 0)
            } else {
                IntoIterInner::Shared(self, 0)
            }
        })
    }
}

enum IntoIterInner<T> {
    Shared(SharedArray<T>, usize),
    // Elements up to the usize member are already moved out
    UnShared(NonNull<SharedArrayInner<T>>, usize),
}

impl<T> Drop for IntoIterInner<T> {
    fn drop(&mut self) {
        match self {
            IntoIterInner::Shared(..) => { /* drop of SharedArray takes care of it */ }
            IntoIterInner::UnShared(inner, begin) => unsafe {
                debug_assert_eq!(inner.as_ref().header.refcount.load(atomic::Ordering::Relaxed), 0);
                let data_ptr = inner.as_ref().data.as_ptr();
                for x in (*begin)..inner.as_ref().header.size {
                    drop(core::ptr::read(data_ptr.add(x)));
                }
                alloc::dealloc(
                    inner.as_ptr() as *mut u8,
                    compute_inner_layout::<T>(inner.as_ref().header.capacity),
                )
            },
        }
    }
}

/// An iterator that moves out of a SharedArray.
///
/// This `struct` is created by the `into_iter` method on [`SharedArray`] (provided
/// by the [`IntoIterator`] trait).
pub struct IntoIter<T>(IntoIterInner<T>);

impl<T: Clone> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IntoIterInner::Shared(array, moved) => {
                let result = array.as_slice().get(*moved).cloned();
                *moved += 1;
                result
            }
            IntoIterInner::UnShared(inner, begin) => unsafe {
                if *begin < inner.as_ref().header.size {
                    let r = core::ptr::read(inner.as_ref().data.as_ptr().add(*begin));
                    *begin += 1;
                    Some(r)
                } else {
                    None
                }
            },
        }
    }
}

#[test]
fn simple_test() {
    let x: SharedArray<i32> = SharedArray::from(&[1, 2, 3]);
    let y: SharedArray<i32> = SharedArray::from(&[3, 2, 1]);
    assert_eq!(x, x.clone());
    assert_ne!(x, y);
    let z: [i32; 3] = [1, 2, 3];
    assert_eq!(z, x.as_slice());
    let vec: Vec<i32> = vec![1, 2, 3];
    assert_eq!(x, vec);
    let def: SharedArray<i32> = Default::default();
    assert_eq!(def, SharedArray::<i32>::default());
    assert_ne!(def, x);
}

pub(crate) mod ffi {
    use super::*;

    #[no_mangle]
    /// This function is used for the low-level C++ interface to allocate the backing vector of a SharedArray.
    pub unsafe extern "C" fn sixtyfps_shared_array_allocate(size: usize, align: usize) -> *mut u8 {
        std::alloc::alloc(std::alloc::Layout::from_size_align(size, align).unwrap())
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to deallocate the backing vector of a SharedArray
    pub unsafe extern "C" fn sixtyfps_shared_array_free(ptr: *mut u8, size: usize, align: usize) {
        std::alloc::dealloc(ptr, std::alloc::Layout::from_size_align(size, align).unwrap())
    }

    #[no_mangle]
    /// This function is used for the low-level C++ interface to initialize the empty SharedArray.
    pub unsafe extern "C" fn sixtyfps_shared_array_empty() -> *const u8 {
        &SHARED_NULL as *const _ as *const u8
    }
}
