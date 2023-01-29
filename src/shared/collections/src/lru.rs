// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::hash_map::HashMap;
use core::{
    alloc::{AllocError, Allocator, Layout},
    borrow::Borrow,
    hash::{BuildHasher, Hash, Hasher},
    num::NonZeroUsize,
    ptr::{addr_of, addr_of_mut, NonNull},
};

/// A fixed-capacity cache with a least-recently-used eviction policy
pub struct LruCache<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    map: HashMap<A, K, NonNull<LruEntry<V>>, S>,
    head: Option<NonNull<LruEntry<V>>>,
    tail: Option<NonNull<LruEntry<V>>>,
    cache_capacity: usize,
}

impl<A, K, V, S> LruCache<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher + ~const Default,
{
    /// Create a new [`LruCache`] with the given allocator and cache capacity
    pub const fn new(allocator: A, cache_capacity: NonZeroUsize) -> Self {
        Self { map: HashMap::new(allocator), head: None, tail: None, cache_capacity: cache_capacity.get() }
    }
}

impl<A, K, V, S> LruCache<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    /// The current number of elements contained within the cache
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns `true` if the cache has no entries
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Create a new [`LruCache`] with the given allocator, hash builder, and
    /// cache capacity
    pub fn with_hasher(allocator: A, hash_builder: S, cache_capacity: NonZeroUsize) -> Self {
        Self {
            map: HashMap::with_hasher(allocator, hash_builder),
            head: None,
            tail: None,
            cache_capacity: cache_capacity.get(),
        }
    }

    /// Insert a new entry into the cache, and returning either:
    ///
    /// `Some(old_value)` if the key was already present, replacing the old
    /// value with the new one
    ///
    /// `Some(least_recently_used)` if the cache is at capacity, evicting the
    /// least recently used entry in the cache
    ///
    /// `None` if the key was not previously present in the cache
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, AllocError> {
        match self.map.get_mut(&key) {
            Some(current_value) => {
                let entry = *current_value;
                unsafe { self.make_head(entry) };
                Ok(Some(core::mem::replace(unsafe { LruEntry::value_mut(entry) }, value)))
            }
            None => {
                let mut removed = None;
                // If we're at capacity, evict the LRU element
                if self.map.len() == self.cache_capacity {
                    let tail = self.tail.unwrap();
                    let hash = unsafe { LruEntry::key_hash(tail) };

                    self.tail = unsafe { LruEntry::prev(tail) };
                    unsafe { LruEntry::detach(tail) };

                    let raw_entry_builder = self.map.raw_entry_mut()?;
                    let raw_entry =
                        raw_entry_builder.from_hash_and_value(hash, |_, v| core::ptr::eq(v.as_ptr(), tail.as_ptr()));
                    let (_, entry_v) = raw_entry.remove().unwrap();

                    let v = unsafe { LruEntry::free(entry_v) };
                    unsafe { self.map.allocator().deallocate(entry_v.cast(), Layout::new::<LruEntry<V>>()) };

                    removed = Some(v);
                }

                let entry: NonNull<LruEntry<V>> = self.map.allocator().allocate(Layout::new::<LruEntry<V>>())?.cast();
                unsafe {
                    entry.as_ptr().write(LruEntry {
                        key_hash: {
                            let mut hasher = self.map.hash_builder().build_hasher();
                            <K as Hash>::hash(&key, &mut hasher);
                            hasher.finish()
                        },
                        value,
                        prev: None,
                        next: None,
                    })
                };

                unsafe { self.make_head(entry) };
                self.map.insert(key, entry)?;

                Ok(removed)
            }
        }
    }

    /// Get a shared reference to the value for `key`, if it exists. This method
    /// will make the entry for `key` the most recently used cache entry.
    pub fn get<Q>(&mut self, key: &Q) -> Option<&V>
    where
        Q: Hash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        match self.map.get(key) {
            Some(entry) => {
                let entry = *entry;
                unsafe { self.make_head(entry) };
                Some(unsafe { LruEntry::value(entry) })
            }
            None => None,
        }
    }

    /// Get a unique reference to the value for `key`, if it exists. This method
    /// will make the entry for `key` the most recently used cache entry.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        Q: Hash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        match self.map.get_mut(key) {
            Some(entry) => {
                let entry = *entry;
                unsafe { self.make_head(entry) };
                Some(unsafe { LruEntry::value_mut(entry) })
            }
            None => None,
        }
    }

    /// Remove the entry for `key`, if it exists, returning the value for the
    /// entry
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: Hash + Eq + ?Sized,
        K: Borrow<Q>,
    {
        match self.map.remove(key) {
            Some(entry) => {
                if self.head == Some(entry) {
                    if self.head == self.tail {
                        // if head and tail are the same, `self.head` will be
                        // set to `None` below, so no need to set it here
                        self.tail = None;
                    }

                    self.head = unsafe { LruEntry::next(entry) };
                } else if self.tail == Some(entry) {
                    self.tail = unsafe { LruEntry::prev(entry) };
                }

                unsafe { LruEntry::detach(entry) };
                let ret = Some(unsafe { LruEntry::free(entry) });

                unsafe { self.map.allocator().deallocate(entry.cast(), Layout::new::<LruEntry<V>>()) };

                ret
            }
            None => None,
        }
    }

    /// Iterator over the values in the cache by descending usage (e.g.
    /// most-recently-used first) without modifying the usage order
    pub fn values_descending(&self) -> ValuesDescending<'_, V> {
        ValuesDescending { entry: self.head, _p: core::marker::PhantomData }
    }

    /// Iterator over the keys in the cache by descending usage (e.g.
    /// most-recently-used first) without modifying the usage order
    pub fn keys_descending(&self) -> KeysDescending<'_, A, K, V, S> {
        KeysDescending { entry: self.head, map: &self.map }
    }

    // /// Iterator over the entries in the cache by descending usage (e.g.
    // /// most-recently-used first) without modifying the usage order
    // pub fn entries_descending(&self) -> EntriesDescending<'_> {
    //     EntriesDescending { head: self.head, _p: core::marker::PhantomData }
    // }

    /// Iterator over the values in the cache by ascending usage (e.g.
    /// least-recently-used first) without modifying the usage order
    pub fn values_ascending(&self) -> ValuesAscending<'_, V> {
        ValuesAscending { entry: self.tail, _p: core::marker::PhantomData }
    }

    /// Iterator over the keys in the cache by ascending usage (e.g.
    /// least-recently-used first) without modifying the usage order
    pub fn keys_ascending(&self) -> KeysAscending<'_, A, K, V, S> {
        KeysAscending { entry: self.tail, map: &self.map }
    }

    // /// Iterator over the entries in the cache by ascending usage (e.g.
    // /// least-recently-used first) without modifying the usage order
    // pub fn entries_ascending(&self) -> EntriesAscending<'_> {
    //     EntriesDescending { tail: self.tail, _p: core::marker::PhantomData }
    // }

    /// Iterator over unique references to the values in the cache by descending
    /// usage (e.g. most-recently-used first) without modifying the usage order
    pub fn values_descending_mut(&mut self) -> ValuesDescendingMut<'_, V> {
        ValuesDescendingMut { entry: self.head, _p: core::marker::PhantomData }
    }

    // /// Iterator over the entries with a unique reference to the values in the
    // /// cache by descending usage (e.g. most-recently-used first) without
    // /// modifying the usage order
    // pub fn entries_descending_mut(&self) -> EntriesDescendingMut<'_> {
    //     EntriesDescendingMut { head: self.head, _p: core::marker::PhantomData }
    // }

    /// Iterator over unique references to the values in the cache by ascending
    /// usage (e.g. least-recently-used first) without modifying the usage order
    pub fn values_ascending_mut(&mut self) -> ValuesAscendingMut<'_, V> {
        ValuesAscendingMut { entry: self.tail, _p: core::marker::PhantomData }
    }

    // /// Iterator over the entries with a unique reference to the values in the
    // /// cache by ascending usage (e.g. least-recently-used first) without
    // /// modifying the usage order
    // pub fn entries_ascending_mut(&self) -> EntriesAscendingMut<'_> {
    //     EntriesDescendingMut { tail: self.tail, _p: core::marker::PhantomData }
    // }

    unsafe fn make_head(&mut self, entry: NonNull<LruEntry<V>>) {
        if self.head == Some(entry) {
            return;
        }

        unsafe { LruEntry::detach(entry) };

        match self.head {
            Some(head) => {
                match self.head == self.tail {
                    false => unsafe { LruEntry::set_prev(head, Some(entry)) },
                    true => {
                        unsafe { LruEntry::set_prev(head, Some(entry)) };
                        self.tail = self.head;
                    }
                }

                unsafe { LruEntry::set_next(entry, Some(head)) };
                self.head = Some(entry);
            }
            None => {
                self.head = Some(entry);
                self.tail = Some(entry);
            }
        }
    }
}

impl<A, K, V, S> Drop for LruCache<A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    fn drop(&mut self) {
        let mut node = self.head;
        while let Some(entry) = node {
            unsafe { LruEntry::free(entry) };
            node = unsafe { LruEntry::next(entry) };

            unsafe { self.map.allocator().deallocate(entry.cast(), Layout::new::<LruEntry<V>>()) };
        }
    }
}

/// See [`LruCache::values_descending`]
pub struct ValuesDescending<'a, V: 'a> {
    entry: Option<NonNull<LruEntry<V>>>,
    _p: core::marker::PhantomData<&'a ()>,
}

impl<'a, V: 'a> Iterator for ValuesDescending<'a, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.entry?;

        let val = Some(unsafe { LruEntry::value(entry) });
        self.entry = unsafe { LruEntry::next(entry) };

        val
    }
}

/// See [`LruCache::values_ascending`]
pub struct ValuesAscending<'a, V: 'a> {
    entry: Option<NonNull<LruEntry<V>>>,
    _p: core::marker::PhantomData<&'a ()>,
}

impl<'a, V: 'a> Iterator for ValuesAscending<'a, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.entry?;

        let val = Some(unsafe { LruEntry::value(entry) });
        self.entry = unsafe { LruEntry::prev(entry) };

        val
    }
}

/// See [`LruCache::values_descending_mut`]
pub struct ValuesDescendingMut<'a, V: 'a> {
    entry: Option<NonNull<LruEntry<V>>>,
    _p: core::marker::PhantomData<&'a mut ()>,
}

impl<'a, V: 'a> Iterator for ValuesDescendingMut<'a, V> {
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.entry?;

        let val = Some(unsafe { LruEntry::value_mut(entry) });
        self.entry = unsafe { LruEntry::next(entry) };

        val
    }
}

/// See [`LruCache::values_ascending_mut`]
pub struct ValuesAscendingMut<'a, V: 'a> {
    entry: Option<NonNull<LruEntry<V>>>,
    _p: core::marker::PhantomData<&'a mut ()>,
}

impl<'a, V: 'a> Iterator for ValuesAscendingMut<'a, V> {
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.entry?;

        let val = Some(unsafe { LruEntry::value_mut(entry) });
        self.entry = unsafe { LruEntry::prev(entry) };

        val
    }
}

/// See [`LruCache::keys_descending`]
pub struct KeysDescending<'a, A, K: 'a, V: 'a, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    entry: Option<NonNull<LruEntry<V>>>,
    map: &'a HashMap<A, K, NonNull<LruEntry<V>>, S>,
}

impl<'a, A, K: 'a, V: 'a, S> Iterator for KeysDescending<'a, A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.entry?;

        let val = Some({
            let hash = unsafe { LruEntry::key_hash(entry) };
            self.map.raw_entry().from_hash_and_value(hash, |_, v| core::ptr::eq(v.as_ptr(), entry.as_ptr())).unwrap().0
        });
        self.entry = unsafe { LruEntry::next(entry) };

        val
    }
}

/// See [`LruCache::keys_ascending`]
pub struct KeysAscending<'a, A, K: 'a, V: 'a, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    entry: Option<NonNull<LruEntry<V>>>,
    map: &'a HashMap<A, K, NonNull<LruEntry<V>>, S>,
}

impl<'a, A, K: 'a, V: 'a, S> Iterator for KeysAscending<'a, A, K, V, S>
where
    A: Allocator,
    K: Eq + Hash,
    S: BuildHasher,
{
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.entry?;

        let val = Some({
            let hash = unsafe { LruEntry::key_hash(entry) };
            self.map.raw_entry().from_hash_and_value(hash, |_, v| core::ptr::eq(v.as_ptr(), entry.as_ptr())).unwrap().0
        });
        self.entry = unsafe { LruEntry::prev(entry) };

        val
    }
}

struct LruEntry<V> {
    key_hash: u64,
    value: V,
    prev: Option<NonNull<Self>>,
    next: Option<NonNull<Self>>,
}

impl<V> LruEntry<V> {
    unsafe fn free(this: NonNull<Self>) -> V {
        let Self { value, .. } = unsafe { this.as_ptr().read() };
        value
    }

    unsafe fn key_hash(this: NonNull<Self>) -> u64 {
        unsafe { *addr_of!((*this.as_ptr()).key_hash) }
    }

    unsafe fn value<'a>(this: NonNull<Self>) -> &'a V {
        unsafe { &*addr_of!((*this.as_ptr()).value) }
    }

    unsafe fn value_mut<'a>(this: NonNull<Self>) -> &'a mut V {
        unsafe { &mut *addr_of_mut!((*this.as_ptr()).value) }
    }

    unsafe fn prev(this: NonNull<Self>) -> Option<NonNull<Self>> {
        unsafe { *addr_of!((*this.as_ptr()).prev) }
    }

    unsafe fn next(this: NonNull<Self>) -> Option<NonNull<Self>> {
        unsafe { *addr_of!((*this.as_ptr()).next) }
    }

    unsafe fn set_prev(this: NonNull<Self>, to: Option<NonNull<Self>>) {
        unsafe { addr_of_mut!((*this.as_ptr()).prev).write(to) };
    }

    unsafe fn set_next(this: NonNull<Self>, to: Option<NonNull<Self>>) {
        unsafe { addr_of_mut!((*this.as_ptr()).next).write(to) };
    }

    unsafe fn detach(this: NonNull<Self>) {
        let prev = unsafe { Self::prev(this) };
        let next = unsafe { Self::next(this) };

        match (prev, next) {
            (Some(prev), Some(next)) => {
                unsafe { Self::set_next(prev, Some(next)) };
                unsafe { Self::set_prev(next, Some(prev)) };
            }
            (Some(prev), None) => unsafe { Self::set_next(prev, None) },
            (None, Some(next)) => unsafe { Self::set_prev(next, None) },
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{alloc::Global, string::String};

    use super::*;
    use crate::hash::FxBuildHasher;

    #[test]
    fn insert_evicts() {
        let mut lru_cache: LruCache<Global, _, _, FxBuildHasher> = LruCache::new(Global, NonZeroUsize::new(4).unwrap());

        for i in 0..4 {
            assert!(lru_cache.insert(std::format!("{i}"), i).unwrap().is_none());
        }

        for i in 4..8 {
            assert_eq!(lru_cache.insert(std::format!("{i}"), i).unwrap(), Some(i - 4));
        }
    }

    #[test]
    fn get_changes_order() {
        let mut lru_cache: LruCache<Global, _, _, FxBuildHasher> = LruCache::new(Global, NonZeroUsize::new(4).unwrap());
        for i in 0..4 {
            assert!(lru_cache.insert(std::format!("{i}"), i).unwrap().is_none());
        }

        for i in 0..4 {
            assert_eq!(lru_cache.get(&std::format!("{i}")), Some(&i));
        }

        for (n, i) in lru_cache.values_descending().zip((0..4).rev()) {
            assert_eq!(*n, i);
        }
    }

    #[test]
    fn remove() {
        let mut lru_cache: LruCache<Global, _, _, FxBuildHasher> = LruCache::new(Global, NonZeroUsize::new(4).unwrap());
        for i in 0..4 {
            assert!(lru_cache.insert(std::format!("{i}"), i).unwrap().is_none());
        }

        assert_eq!(lru_cache.remove("3").unwrap(), 3);

        for (n, i) in lru_cache.values_descending().zip((0..3).rev()) {
            assert_eq!(*n, i);
        }

        assert_eq!(lru_cache.remove("0").unwrap(), 0);

        for (n, i) in lru_cache.values_descending().zip((1..3).rev()) {
            assert_eq!(*n, i);
        }

        let mut lru_cache: LruCache<Global, _, _, FxBuildHasher> = LruCache::new(Global, NonZeroUsize::new(4).unwrap());
        for i in 0..4 {
            assert!(lru_cache.insert(std::format!("{i}"), i).unwrap().is_none());
        }

        assert_eq!(lru_cache.remove("2").unwrap(), 2);

        for (n, i) in lru_cache.values_descending().zip((0..2).chain(3..4).rev()) {
            assert_eq!(*n, i);
        }
    }

    #[test]
    fn insert() {
        let mut lru_cache: LruCache<Global, _, _, FxBuildHasher> = LruCache::new(Global, NonZeroUsize::new(4).unwrap());
        for i in 0..4 {
            assert!(lru_cache.insert(std::format!("{i}"), i).unwrap().is_none());
        }

        assert_eq!(lru_cache.insert(String::from("1"), 5).unwrap(), Some(1));

        for (n, i) in lru_cache.values_descending().zip([5, 3, 2, 0]) {
            assert_eq!(*n, i);
        }
    }

    #[test]
    fn iters() {
        let mut lru_cache: LruCache<Global, _, _, FxBuildHasher> = LruCache::new(Global, NonZeroUsize::new(4).unwrap());
        for i in 0..4 {
            assert!(lru_cache.insert(std::format!("{i}"), i).unwrap().is_none());
        }

        for (n, i) in lru_cache.values_descending().zip((0..4).rev()) {
            assert_eq!(*n, i);
        }

        for (n, i) in lru_cache.values_descending_mut().zip((0..4).rev()) {
            assert_eq!(*n, i);
        }

        for (n, i) in lru_cache.values_ascending().zip(0..4) {
            assert_eq!(*n, i);
        }

        for (n, i) in lru_cache.values_ascending_mut().zip(0..4) {
            assert_eq!(*n, i);
        }

        for (n, i) in lru_cache.keys_ascending().zip(0..4) {
            assert_eq!(n, &std::format!("{i}"));
        }

        for (n, i) in lru_cache.keys_descending().zip((0..4).rev()) {
            assert_eq!(n, &std::format!("{i}"));
        }
    }
}
