// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{
    alloc::{AllocError, Allocator, Layout},
    borrow::Borrow,
    hash::{BuildHasher, Hash, Hasher},
    mem::MaybeUninit,
    ptr::{addr_of, addr_of_mut, NonNull},
};

const LOAD_FACTOR_LIMIT: usize = 70;

/// An open-addressed with quadratic probing hash table implementation
pub struct HashMap<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    allocator: A,
    hash_builder: S,
    bucket: NonNull<[Slot<K, V>]>,
    len: usize,
}

impl<A, K, V, S> HashMap<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Create a new [`HashMap`] with the given allocator
    pub const fn new(allocator: A, hash_builder: S) -> Self {
        Self { allocator, hash_builder, len: 0, bucket: NonNull::slice_from_raw_parts(NonNull::dangling(), 0) }
    }
}

impl<A, K, V, S> HashMap<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    /// Attempt to immediately reserve at least `capacity` for the [`HashMap`]
    /// before creating it
    pub fn with_capacity(allocator: A, capacity: usize) -> Result<Self, AllocError> {
        if capacity >= (isize::MAX as usize) {
            return Err(AllocError);
        }

        let bucket = allocator.allocate_zeroed(Layout::array::<Slot<K, V>>(capacity).map_err(|_| AllocError)?)?;
        let bucket = NonNull::slice_from_raw_parts(bucket.as_non_null_ptr().cast(), capacity);

        Ok(Self { allocator, hash_builder: S::default(), len: 0, bucket })
    }
}

impl<A, K, V, S> HashMap<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Create a new [`HashMap`] with the given allocator and hash builder
    pub fn with_hasher(allocator: A, hash_builder: S) -> Self {
        Self { allocator, hash_builder, len: 0, bucket: NonNull::slice_from_raw_parts(NonNull::dangling(), 0) }
    }

    /// A shared reference to the [`HashMap`]'s allocator instance
    #[inline(always)]
    pub const fn allocator(&self) -> &A {
        &self.allocator
    }

    /// The [`BuildHasher`] instance of the [`HashMap`]
    #[inline(always)]
    pub const fn hash_builder(&self) -> &S {
        &self.hash_builder
    }

    /// The current capacity of the [`HashMap`]
    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        self.bucket.len()
    }

    /// The number of occupied entries inside of the [`HashMap`]
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Insert a new key and value into the [`HashMap`], returning the previous
    /// entry's value, if the key was previously inserted
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, AllocError> {
        if self.capacity() == 0 {
            self.init(4)?;
        }
        if self.load_factor() > LOAD_FACTOR_LIMIT {
            self.resize(self.capacity().saturating_mul(2))?;
        }

        let slot = unsafe { self.slot_for_key(&key) };
        let (slot, ret) = match slot {
            RawBucketSlot::Occupied(slot) => (slot, Some(unsafe { Slot::free(slot).1 })),
            RawBucketSlot::Vacant(slot) => {
                self.len += 1;
                (slot, None)
            }
        };

        unsafe { Slot::allocate(slot, key, value) };

        Ok(ret)
    }

    /// Look up the given key in the map, returning a shared reference to the
    /// corresponding value, if it exists
    #[inline]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.capacity() == 0 {
            return None;
        }

        match unsafe { self.slot_for_key(key) } {
            RawBucketSlot::Occupied(slot) => Some(unsafe { Slot::value(slot) }),
            RawBucketSlot::Vacant(_) => None,
        }
    }

    /// Look up the given key in the map, returning a unique reference to the
    /// corresponding value, if it exists
    #[inline]
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.capacity() == 0 {
            return None;
        }

        match unsafe { self.slot_for_key(key) } {
            RawBucketSlot::Occupied(slot) => Some(unsafe { Slot::value_mut(slot) }),
            RawBucketSlot::Vacant(_) => None,
        }
    }

    /// Attempt to remove an entry from the [`HashMap`], returning the value of
    /// the entry if it was present
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.capacity() == 0 {
            return None;
        }

        match unsafe { self.slot_for_key(key) } {
            RawBucketSlot::Occupied(slot) => {
                self.len -= 1;

                let (_, value) = unsafe { Slot::free(slot) };
                Some(value)
            }
            RawBucketSlot::Vacant(_) => None,
        }
    }

    /// Retrieve the [`Entry`] for the given key, allowing modification &
    /// insertion of a value at the same time
    #[inline]
    pub fn entry(&mut self, key: K) -> Result<Entry<'_, K, V>, AllocError> {
        if self.capacity() == 0 {
            self.init(4)?;
        }

        self.reserve(1)?;

        match unsafe { self.slot_for_key(&key) } {
            RawBucketSlot::Occupied(slot) => Ok(Entry::Occupied(OccupiedEntry { slot, _p: core::marker::PhantomData })),
            RawBucketSlot::Vacant(slot) => Ok(Entry::Vacant(VacantEntry { slot, key, len: &mut self.len })),
        }
    }

    /// Obtain a [`RawEntryBuilder`] which allows for working with raw
    /// [`HashMap`] entries. The raw entry APIs do not require an owned key to
    /// access and view already-populated entries within the map, allowing for memoization of hashes
    #[inline]
    pub fn raw_entry(&self) -> RawEntryBuilder<'_, A, K, V, S> {
        RawEntryBuilder { map: self }
    }

    /// Obtain a [`RawEntryBuilderMut`] which allows for working with raw
    /// [`HashMap`] entries. The raw entry APIs do not require an owned key to
    /// access and view already-populated entries within the map, allowing for memoization of hashes
    #[inline]
    pub fn raw_entry_mut(&mut self) -> Result<RawEntryBuilderMut<'_, A, K, V, S>, AllocError> {
        self.reserve(1)?;

        Ok(RawEntryBuilderMut { map: self })
    }

    /// Attempt to reserve at least `additional` items in the [`HashMap`],
    /// potentially allocating more to reduce the number of subsequent
    /// allocations.
    #[inline]
    pub fn reserve(&mut self, additional: usize) -> Result<(), AllocError> {
        if self.len().saturating_add(additional) <= self.capacity() {
            return Ok(());
        }

        self.resize(self.len().saturating_add(additional))
    }

    // Safety: this function must only be called when `self.bucket` is a valid
    // region of memory (AKA initialized by `self.init()`)
    unsafe fn slot_for_key<Q>(&self, key: &Q) -> RawBucketSlot<K, V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut hasher = self.hash_builder.build_hasher();
        Q::hash(key, &mut hasher);
        let hash = hasher.finish();
        unsafe {
            self.slot_for_hash(hash, |k| {
                let k = unsafe { Slot::key(k) };
                k.borrow() == key
            })
        }
    }

    // Safety: this function must only be called when `self.bucket` is a valid
    // region of memory (AKA initialized by `self.init()`)
    unsafe fn slot_for_hash<F>(&self, hash: u64, mut eq: F) -> RawBucketSlot<K, V>
    where
        F: FnMut(NonNull<Slot<K, V>>) -> bool,
    {
        let hash = usize::try_from(hash).unwrap();
        let mut index = hash % self.capacity();
        let mut ptr = unsafe { self.bucket.get_unchecked_mut(index) };
        let mut attempt = 1usize;

        while unsafe { Slot::occupied(ptr) } {
            if eq(ptr) {
                return RawBucketSlot::Occupied(ptr);
            }

            index = Self::h(hash, attempt, self.capacity());
            attempt += 1;

            if index >= self.capacity() {
                continue;
            }

            ptr = unsafe { self.bucket.get_unchecked_mut(index) };
        }

        RawBucketSlot::Vacant(ptr)
    }

    #[cold]
    fn resize(&mut self, to: usize) -> Result<(), AllocError> {
        if self.capacity() == 0 {
            return self.init(to);
        }

        let new_capacity = self.capacity().saturating_mul(2).max(to);

        if new_capacity >= (isize::MAX as usize) {
            return Err(AllocError);
        }

        let new_bucket =
            self.allocator.allocate_zeroed(Layout::array::<Slot<K, V>>(new_capacity).map_err(|_| AllocError)?)?;
        let new_bucket = NonNull::slice_from_raw_parts(new_bucket.as_non_null_ptr().cast(), new_capacity);

        for index in 0..self.capacity() {
            let current_ptr = unsafe { self.bucket.get_unchecked_mut(index) };

            if unsafe { Slot::occupied(current_ptr) } {
                let mut hasher = self.hash_builder.build_hasher();
                K::hash(unsafe { Slot::key(current_ptr) }, &mut hasher);
                let hash = usize::try_from(hasher.finish()).unwrap();

                let mut index = hash % new_capacity;
                let mut new_ptr = unsafe { new_bucket.get_unchecked_mut(index) };
                let mut attempt = 1usize;

                while unsafe { Slot::occupied(new_ptr) } {
                    index = Self::h(hash, attempt, new_capacity);
                    attempt += 1;

                    if index >= new_capacity {
                        continue;
                    }

                    new_ptr = unsafe { new_bucket.get_unchecked_mut(index) };
                }

                unsafe { core::ptr::copy_nonoverlapping(current_ptr.as_ptr(), new_ptr.as_ptr(), 1) };
            }
        }

        unsafe { self.allocator.deallocate(self.bucket.cast(), Layout::array::<Slot<K, V>>(self.capacity()).unwrap()) };
        self.bucket = new_bucket;

        Ok(())
    }

    #[cold]
    fn init(&mut self, requested: usize) -> Result<(), AllocError> {
        let bucket = self
            .allocator
            .allocate_zeroed(Layout::array::<Slot<K, V>>(requested.next_power_of_two()).map_err(|_| AllocError)?)?;
        let bucket = NonNull::slice_from_raw_parts(bucket.as_non_null_ptr().cast(), requested.next_power_of_two());

        self.bucket = bucket;

        Ok(())
    }

    // From https://en.wikipedia.org/wiki/Quadratic_probing
    // For any m, full cycle with quadratic probing can be achieved by rounding up m to closest power of 2, compute probe index:
    //    h(k, i) = h(k) + ((i^2 + i) / 2) % roundUp2(m)
    // where `k` is the key, `h(k)` is the hash of the key, `i` is the iteration count, and `roundUp2(m)` is the next power of 2
    // of the array length.
    #[inline(always)]
    fn h(hash: usize, attempt: usize, capacity: usize) -> usize {
        (hash + (attempt.wrapping_pow(2) + attempt) / 2) % capacity.next_power_of_two()
    }

    fn load_factor(&self) -> usize {
        self.len.saturating_mul(100) / self.bucket.len()
    }
}

impl<A, K, V, S> Drop for HashMap<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    fn drop(&mut self) {
        for index in 0..self.capacity() {
            let slot = unsafe { self.bucket.get_unchecked_mut(index) };
            if unsafe { Slot::occupied(slot) } {
                unsafe { Slot::free(slot) };
            }
        }

        if self.capacity() > 0 {
            unsafe {
                self.allocator.deallocate(self.bucket.cast(), Layout::array::<Slot<K, V>>(self.capacity()).unwrap())
            };
        }
    }
}

/// A [`HashMap`] entry, which is either occupied or vacant
#[allow(missing_docs)]
pub enum Entry<'a, K: 'a, V: 'a> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K: 'a, V: 'a> Entry<'a, K, V> {
    /// The corresponding key for this [`Entry`]
    pub fn key(&self) -> &K {
        match self {
            Self::Occupied(occupied) => unsafe { Slot::key(occupied.slot) },
            Self::Vacant(vacant) => &vacant.key,
        }
    }

    /// Modify the value of this [`Entry`], if it exists, before doing any
    /// insertions
    pub fn and_modify<F: FnOnce(&mut V)>(self, f: F) -> Self {
        match &self {
            Self::Occupied(occupied) => {
                f(unsafe { Slot::value_mut(occupied.slot) });
                self
            }
            Self::Vacant(_) => self,
        }
    }

    /// Retrieve a unique reference to the present [`Entry`]'s value, or insert
    /// a new value and return a unique reference to that
    pub fn or_insert(self, value: V) -> &'a mut V {
        match self {
            Self::Occupied(occupied) => unsafe { Slot::value_mut(occupied.slot) },
            Self::Vacant(vacant) => {
                *vacant.len += 1;
                unsafe { Slot::allocate(vacant.slot, vacant.key, value) };
                unsafe { Slot::value_mut(vacant.slot) }
            }
        }
    }

    /// [`Entry::or_insert`] but takes a closure for lazily creating the entry
    /// value only if the entry does not exist
    pub fn or_insert_with<F: FnOnce() -> V>(self, f: F) -> &'a mut V {
        match self {
            Self::Occupied(occupied) => unsafe { Slot::value_mut(occupied.slot) },
            Self::Vacant(vacant) => {
                // In the case of panics, don't actually mark the slot as
                // allocated
                let value = f();
                *vacant.len += 1;
                unsafe { Slot::allocate(vacant.slot, vacant.key, value) };
                unsafe { Slot::value_mut(vacant.slot) }
            }
        }
    }

    /// [`Entry::or_insert_with`] but passes a shared reference to the
    /// [`Entry`]'s key to allow for value creation based on the key, without
    /// needing to keep an extra copy of the key's value before using
    /// [`HashMap::entry`]
    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, f: F) -> &'a mut V {
        match self {
            Self::Occupied(occupied) => unsafe { Slot::value_mut(occupied.slot) },
            Self::Vacant(vacant) => {
                // In the case of panics, don't actually mark the slot as
                // allocated
                let value = f(&vacant.key);
                *vacant.len += 1;
                unsafe { Slot::allocate(vacant.slot, vacant.key, value) };
                unsafe { Slot::value_mut(vacant.slot) }
            }
        }
    }
}

impl<'a, K: 'a, V: Default + 'a> Entry<'a, K, V> {
    /// Retrieve a unique reference to the present [`Entry`]'s value, or insert
    /// the default value for `V` and return a unique reference to that
    pub fn or_default(self) -> &'a mut V {
        match self {
            Self::Occupied(occupied) => unsafe { Slot::value_mut(occupied.slot) },
            Self::Vacant(vacant) => {
                let value = V::default();
                *vacant.len += 1;
                unsafe { Slot::allocate(vacant.slot, vacant.key, value) };
                unsafe { Slot::value_mut(vacant.slot) }
            }
        }
    }
}

/// An occupied entry in a [`HashMap`]
pub struct OccupiedEntry<'a, K, V> {
    slot: NonNull<Slot<K, V>>,
    _p: core::marker::PhantomData<&'a mut ()>,
}

/// A vacant entry in a [`HashMap`]
pub struct VacantEntry<'a, K, V> {
    slot: NonNull<Slot<K, V>>,
    key: K,
    len: &'a mut usize,
}

/// Helper struct for working with raw [`HashMap`] entries
pub struct RawEntryBuilder<'a, A: 'a, K: 'a, V: 'a, S: 'a>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    map: &'a HashMap<A, K, V, S>,
}

impl<'a, A, K, V, S> RawEntryBuilder<'a, A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Retrieve a shared reference to the entry corresponding to `key`, if it
    /// exists
    #[inline]
    pub fn from_key<Q>(self, key: &Q) -> Option<(&'a K, &'a V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        if self.map.capacity() == 0 {
            return None;
        }

        match unsafe { self.map.slot_for_key(key) } {
            RawBucketSlot::Occupied(slot) => Some((unsafe { Slot::key(slot) }, unsafe { Slot::value(slot) })),
            RawBucketSlot::Vacant(_) => None,
        }
    }

    /// Retrieve a shared reference to the key and value for the entry
    /// corresponding to the prehashed key, checking for equality with `key`, if
    /// it exists
    #[inline]
    pub fn from_prehashed_key<Q>(self, hash: u64, key: &Q) -> Option<(&'a K, &'a V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        if self.map.capacity() == 0 {
            return None;
        }

        match unsafe {
            self.map.slot_for_hash(hash, |slot| {
                let k = unsafe { Slot::key(slot) };
                k.borrow() == key
            })
        } {
            RawBucketSlot::Occupied(slot) => Some((unsafe { Slot::key(slot) }, unsafe { Slot::value(slot) })),
            RawBucketSlot::Vacant(_) => None,
        }
    }

    #[inline]
    pub fn from_hash<F>(self, hash: u64, mut is_match: F) -> Option<(&'a K, &'a V)>
    where
        F: FnMut(&K) -> bool,
    {
        if self.map.capacity() == 0 {
            return None;
        }

        match unsafe {
            self.map.slot_for_hash(hash, |slot| {
                let k = unsafe { Slot::key(slot) };
                is_match(k)
            })
        } {
            RawBucketSlot::Occupied(slot) => Some((unsafe { Slot::key(slot) }, unsafe { Slot::value(slot) })),
            RawBucketSlot::Vacant(_) => None,
        }
    }

    #[inline]
    pub fn from_hash_and_value<F>(self, hash: u64, mut is_match: F) -> Option<(&'a K, &'a V)>
    where
        F: FnMut(&K, &V) -> bool,
    {
        if self.map.capacity() == 0 {
            return None;
        }

        match unsafe {
            self.map.slot_for_hash(hash, |slot| {
                let (k, v) = unsafe { (Slot::key(slot), Slot::value(slot)) };
                is_match(k, v)
            })
        } {
            RawBucketSlot::Occupied(slot) => Some((unsafe { Slot::key(slot) }, unsafe { Slot::value(slot) })),
            RawBucketSlot::Vacant(_) => None,
        }
    }
}

pub struct RawEntryBuilderMut<'a, A: 'a, K: 'a, V: 'a, S: 'a>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    map: &'a mut HashMap<A, K, V, S>,
}

impl<'a, A, K, V, S> RawEntryBuilderMut<'a, A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    #[inline]
    pub fn from_key<Q>(self, key: &Q) -> RawEntryMut<'a, K, V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        match unsafe { self.map.slot_for_key(key) } {
            RawBucketSlot::Occupied(slot) => {
                RawEntryMut::Occupied(RawOccupiedEntryMut { slot, len: &mut self.map.len })
            }
            RawBucketSlot::Vacant(slot) => RawEntryMut::Vacant(RawVacantEntryMut { slot, len: &mut self.map.len }),
        }
    }

    #[inline]
    pub fn from_prehashed_key<Q>(self, hash: u64, key: &Q) -> RawEntryMut<'a, K, V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        match unsafe {
            self.map.slot_for_hash(hash, |slot| {
                let k = unsafe { Slot::key(slot) };
                k.borrow() == key
            })
        } {
            RawBucketSlot::Occupied(slot) => {
                RawEntryMut::Occupied(RawOccupiedEntryMut { slot, len: &mut self.map.len })
            }
            RawBucketSlot::Vacant(slot) => RawEntryMut::Vacant(RawVacantEntryMut { slot, len: &mut self.map.len }),
        }
    }

    #[inline]
    pub fn from_hash<F>(self, hash: u64, mut is_match: F) -> RawEntryMut<'a, K, V>
    where
        F: FnMut(&K) -> bool,
    {
        match unsafe {
            self.map.slot_for_hash(hash, |slot| {
                let k = unsafe { Slot::key(slot) };
                is_match(k)
            })
        } {
            RawBucketSlot::Occupied(slot) => {
                RawEntryMut::Occupied(RawOccupiedEntryMut { slot, len: &mut self.map.len })
            }
            RawBucketSlot::Vacant(slot) => RawEntryMut::Vacant(RawVacantEntryMut { slot, len: &mut self.map.len }),
        }
    }

    #[inline]
    pub fn from_hash_and_value<F>(self, hash: u64, mut is_match: F) -> RawEntryMut<'a, K, V>
    where
        F: FnMut(&K, &V) -> bool,
    {
        match unsafe {
            self.map.slot_for_hash(hash, |slot| {
                let (k, v) = unsafe { (Slot::key(slot), Slot::value(slot)) };
                is_match(k, v)
            })
        } {
            RawBucketSlot::Occupied(slot) => {
                RawEntryMut::Occupied(RawOccupiedEntryMut { slot, len: &mut self.map.len })
            }
            RawBucketSlot::Vacant(slot) => RawEntryMut::Vacant(RawVacantEntryMut { slot, len: &mut self.map.len }),
        }
    }
}

/// A [`HashMap`] entry, which is either occupied or vacant
#[allow(missing_docs)]
pub enum RawEntryMut<'a, K: 'a, V: 'a> {
    Occupied(RawOccupiedEntryMut<'a, K, V>),
    Vacant(RawVacantEntryMut<'a, K, V>),
}

impl<'a, K: 'a, V: 'a> RawEntryMut<'a, K, V> {
    /// Modify the key and value of this [`RawEntryMut`], if it exists, before
    /// doing any insertions
    pub fn and_modify<F: FnOnce(&mut K, &mut V)>(self, f: F) -> Self {
        match &self {
            Self::Occupied(occupied) => {
                f(unsafe { Slot::key_mut(occupied.slot) }, unsafe { Slot::value_mut(occupied.slot) });
                self
            }
            Self::Vacant(_) => self,
        }
    }

    /// Retrieve a unique reference to the present [`RawEntryMut`]'s key and
    /// value, or insert a new key and value and return a unique reference to
    /// that
    pub fn or_insert(self, key: K, value: V) -> (&'a mut K, &'a mut V) {
        match self {
            Self::Occupied(occupied) => {
                (unsafe { Slot::key_mut(occupied.slot) }, unsafe { Slot::value_mut(occupied.slot) })
            }
            Self::Vacant(vacant) => {
                *vacant.len += 1;
                unsafe { Slot::allocate(vacant.slot, key, value) };
                (unsafe { Slot::key_mut(vacant.slot) }, unsafe { Slot::value_mut(vacant.slot) })
            }
        }
    }

    /// [`RawEntryMut::or_insert`] but takes a closure for lazily creating the
    /// entry key and value only if the entry does not exist
    pub fn or_insert_with<F: FnOnce() -> (K, V)>(self, f: F) -> &'a mut V {
        match self {
            Self::Occupied(occupied) => unsafe { Slot::value_mut(occupied.slot) },
            Self::Vacant(vacant) => {
                // In the case of panics, don't actually mark the slot as
                // allocated
                let (key, value) = f();
                *vacant.len += 1;
                unsafe { Slot::allocate(vacant.slot, key, value) };
                unsafe { Slot::value_mut(vacant.slot) }
            }
        }
    }

    pub fn remove(self) -> Option<(K, V)> {
        match self {
            Self::Occupied(occupied) => {
                *occupied.len -= 1;
                Some(unsafe { Slot::free(occupied.slot) })
            }
            Self::Vacant(_) => None,
        }
    }
}

/// A raw occupied entry in a [`HashMap`]
pub struct RawOccupiedEntryMut<'a, K, V> {
    slot: NonNull<Slot<K, V>>,
    len: &'a mut usize,
}

/// A raw vacant entry in a [`HashMap`]
pub struct RawVacantEntryMut<'a, K, V> {
    slot: NonNull<Slot<K, V>>,
    len: &'a mut usize,
}

enum RawBucketSlot<K, V> {
    Vacant(NonNull<Slot<K, V>>),
    Occupied(NonNull<Slot<K, V>>),
}

struct Slot<K, V> {
    discriminant: u32,
    key: MaybeUninit<K>,
    value: MaybeUninit<V>,
}

impl<K, V> Slot<K, V> {
    const VACANT: u32 = 0;
    const OCCUPIED: u32 = 1;

    unsafe fn occupied(this: NonNull<Self>) -> bool {
        unsafe { addr_of!((*this.as_ptr()).discriminant).read() == Self::OCCUPIED }
    }

    unsafe fn allocate(this: NonNull<Self>, key: K, value: V) {
        unsafe {
            this.as_ptr().write(Self {
                discriminant: Self::OCCUPIED,
                key: MaybeUninit::new(key),
                value: MaybeUninit::new(value),
            })
        }
    }

    unsafe fn free(this: NonNull<Self>) -> (K, V) {
        let Self { key, value, .. } = unsafe { this.as_ptr().read() };
        unsafe { addr_of_mut!((*this.as_ptr()).discriminant).write(Self::VACANT) }

        unsafe { (key.assume_init(), value.assume_init()) }
    }

    unsafe fn key<'a>(this: NonNull<Self>) -> &'a K {
        unsafe { (*addr_of!((*this.as_ptr()).key)).assume_init_ref() }
    }

    unsafe fn key_mut<'a>(this: NonNull<Self>) -> &'a mut K {
        unsafe { (*addr_of_mut!((*this.as_ptr()).key)).assume_init_mut() }
    }

    unsafe fn value<'a>(this: NonNull<Self>) -> &'a V {
        unsafe { (*addr_of!((*this.as_ptr()).value)).assume_init_ref() }
    }

    unsafe fn value_mut<'a>(this: NonNull<Self>) -> &'a mut V {
        unsafe { (*addr_of_mut!((*this.as_ptr()).value)).assume_init_mut() }
    }
}

#[cfg(test)]
mod tests {
    use std::{alloc::Global, string::String};

    use super::*;
    use crate::hash::FxBuildHasher;

    #[derive(Default)]
    struct HorribleHasher(u64);

    impl core::hash::Hasher for HorribleHasher {
        fn write(&mut self, bytes: &[u8]) {
            self.0 += u64::try_from(bytes.len()).unwrap();
        }

        fn finish(&self) -> u64 {
            self.0
        }
    }

    type HorribleBuildHasher = core::hash::BuildHasherDefault<HorribleHasher>;

    #[test]
    fn hash_some_stuff() {
        let mut hashmap: HashMap<Global, _, _, FxBuildHasher> = HashMap::new(Global, FxBuildHasher::default());

        assert!(hashmap.insert(String::from("A hash key!"), 1u32).unwrap().is_none());
        assert!(hashmap.insert(String::from("Another hash key!"), 2u32).unwrap().is_none());
        assert_eq!(hashmap.insert(String::from("A hash key!"), 3u32), Ok(Some(1)));

        let mut hashmap: HashMap<Global, _, _, FxBuildHasher> = HashMap::with_capacity(Global, 1).unwrap();

        assert!(hashmap.insert(String::from("A hash key!"), 1u32).unwrap().is_none());
        assert!(hashmap.insert(String::from("Another hash key!"), 2u32).unwrap().is_none());
        assert_eq!(hashmap.insert(String::from("A hash key!"), 3u32), Ok(Some(1)));
        assert_eq!(hashmap.get("A hash key!"), Some(&3));
        assert_eq!(hashmap.get_mut("A hash key!"), Some(&mut 3));
    }

    #[test]
    fn collision_tests() {
        let mut hashmap: HashMap<Global, _, _, HorribleBuildHasher> = HashMap::new(Global, FxBuildHasher::default());

        assert!(hashmap.insert(String::from("123"), 5u32).unwrap().is_none());
        assert_eq!(hashmap.len(), 1);

        assert_eq!(hashmap.insert(String::from("123"), 6u32).unwrap(), Some(5u32));
        assert_eq!(hashmap.get("123"), Some(&6u32));
        assert_eq!(hashmap.get_mut("123"), Some(&mut 6u32));
        assert_eq!(hashmap.len(), 1);

        assert!(hashmap.insert(String::from("456"), 10u32).unwrap().is_none());
        assert_eq!(hashmap.len(), 2);

        assert_eq!(hashmap.insert(String::from("456"), 11u32).unwrap(), Some(10u32));
        assert_eq!(hashmap.get("456"), Some(&11u32));
        assert_eq!(hashmap.get_mut("456"), Some(&mut 11u32));
        assert_eq!(hashmap.len(), 2);

        let mut hashmap: HashMap<Global, _, _, HorribleBuildHasher> = HashMap::new(Global, FxBuildHasher::default());

        #[cfg(miri)]
        let (range_a, range_b) = (1000..1050, 0..50);

        #[cfg(not(miri))]
        let (range_a, range_b) = (1000..2000, 0..1000);

        for (k, v) in (range_a.clone()).zip(range_b.clone()) {
            assert!(
                hashmap.insert(std::format!("{k}"), v).unwrap().is_none(),
                "failed to insert key={k} with value={v}"
            );
        }

        for (k, v) in (range_a).zip(range_b) {
            let key = std::format!("{k}");
            assert_eq!(hashmap.get(&*key).unwrap(), &v);
        }

        #[cfg(miri)]
        let (range_a, range_b) = (10000..10050, 10000..10050);

        #[cfg(not(miri))]
        let (range_a, range_b) = (10000..11000, 10000..11000);

        for (k, v) in (range_a.clone()).zip(range_b.clone()) {
            assert!(
                hashmap.insert(std::format!("{k}"), v).unwrap().is_none(),
                "failed to insert key={k} with value={v}"
            );
        }

        for (k, v) in (range_a).zip(range_b) {
            let key = std::format!("{k}");
            assert_eq!(hashmap.get(&*key).unwrap(), &v);
        }
    }

    #[test]
    fn entry() -> Result<(), std::boxed::Box<dyn std::error::Error>> {
        let mut hashmap: HashMap<Global, _, _, FxBuildHasher> = HashMap::new(Global, FxBuildHasher::default());

        hashmap.entry(String::from("key1"))?.or_insert(5u32);
        assert_eq!(*hashmap.entry(String::from("key1")).unwrap().and_modify(|n| *n += 1).or_insert(5u32), 6);
        assert_eq!(*hashmap.entry(String::from("key5")).unwrap().and_modify(|n| *n += 1).or_insert(5u32), 5);

        assert_eq!(*hashmap.entry(String::from("key2"))?.or_insert_with_key(|k| k.len() as u32), 4);
        assert_eq!(*hashmap.entry(String::from("key3"))?.or_insert_with(|| 100), 100);
        assert_eq!(*hashmap.entry(String::from("key4"))?.or_default(), 0);
        assert_eq!(*hashmap.entry(String::from("key4"))?.or_default(), 0);

        assert_eq!(hashmap.len(), 5);

        Ok(())
    }

    #[test]
    fn get_get_mut() {
        let mut hashmap: HashMap<Global, String, u32, FxBuildHasher> = HashMap::new(Global, FxBuildHasher::default());
        assert!(hashmap.get("fraz").is_none());
        assert!(hashmap.get_mut("fraz").is_none());
    }

    #[test]
    fn reserve() {
        let mut hashmap: HashMap<Global, String, u32, FxBuildHasher> = HashMap::new(Global, FxBuildHasher::default());
        hashmap.reserve(10).unwrap();
        assert!(hashmap.capacity() >= 10);
        assert_eq!(hashmap.len(), 0);
    }
}
