// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{
    alloc::{AllocError, Allocator, Layout},
    iter::FusedIterator,
    ops::{Bound, Range},
    ptr::{addr_of, addr_of_mut, NonNull},
};

use super::AllocatorCanMerge;

/// The range supplied to [`DoublyLinkedList::drain`] was not valid for the
/// given [`DoublyLinkedList`]
#[derive(Debug, Clone, Copy)]
pub struct DrainRangeError;

#[derive(Debug)]
pub enum InsertError<T> {
    AllocError(AllocError),
    InvalidIndex(T),
}

impl<T> From<AllocError> for InsertError<T> {
    fn from(v: AllocError) -> Self {
        Self::AllocError(v)
    }
}

pub struct DoublyLinkedList<A: Allocator, T> {
    allocator: A,
    head: Option<NonNull<Node<T>>>,
    tail: Option<NonNull<Node<T>>>,
    len: usize,
}

impl<A: Allocator, T> DoublyLinkedList<A, T> {
    pub fn new(allocator: A) -> Self {
        Self { allocator, head: None, tail: None, len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push_front(&mut self, value: T) -> Result<(), AllocError> {
        let node = Node::new(&self.allocator, value, None, self.head)?;

        if self.head.is_none() {
            self.tail = Some(node);
        }

        self.head = Some(node);
        self.len += 1;

        Ok(())
    }

    pub fn push_back(&mut self, value: T) -> Result<(), AllocError> {
        let node = Node::new(&self.allocator, value, self.tail, None)?;

        if self.head.is_none() {
            self.head = Some(node);
        }

        self.tail = Some(node);
        self.len += 1;

        Ok(())
    }

    pub fn insert(&mut self, at: usize, value: T) -> Result<(), InsertError<T>> {
        if at == 0 {
            return Ok(self.push_front(value)?);
        } else if at == self.len {
            return Ok(self.push_back(value)?);
        } else if at > self.len {
            return Err(InsertError::InvalidIndex(value));
        }

        let node = self.node_at(at).unwrap();
        let node_ptr = node.as_ptr();
        let prev = unsafe { addr_of!((*node_ptr).prev).read() };

        self.len += 1;

        Node::new(&self.allocator, value, prev, Some(node))?;

        Ok(())
    }

    /// Extend the [`DoublyLinkedList`] from an iterator. This is a non-trait
    /// method version of the [`Extend::extend`] method that will not panic on
    /// allocation failure.
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) -> Result<(), AllocError> {
        let mut iterator = iter.into_iter();
        let mut last_node = match self.tail {
            Some(node) => node,
            None => {
                let Some(value) = iterator.next() else { return Ok(()) };
                self.push_back(value)?;
                self.head.unwrap()
            }
        };

        for value in iterator {
            let new_node = Node::new(&self.allocator, value, Some(last_node), None)?;
            last_node = new_node;
            self.len += 1;
        }

        Ok(())
    }

    pub fn pop_front(&mut self) -> Option<T> {
        let head = self.head?;
        let head_ptr = head.as_ptr();

        match unsafe { addr_of!((*head_ptr).next).read() } {
            Some(next) => {
                self.head = Some(next);
            }
            // head == tail, so reset both
            None => {
                self.head = None;
                self.tail = None;
            }
        }

        self.len -= 1;

        let value = unsafe { addr_of!((*head_ptr).value).read() };
        unsafe { Node::drop(head, &self.allocator) };

        Some(value)
    }

    pub fn pop_back(&mut self) -> Option<T> {
        let tail = self.tail?;
        let tail_ptr = tail.as_ptr();

        match unsafe { addr_of!((*tail_ptr).prev).read() } {
            Some(prev) => {
                let prev_ptr = prev.as_ptr();
                unsafe { addr_of_mut!((*prev_ptr).next).write(None) };

                self.tail = Some(prev);
            }
            // head == tail, so reset both
            None => {
                self.head = None;
                self.tail = None;
            }
        }

        self.len -= 1;

        let value = unsafe { addr_of!((*tail_ptr).value).read() };
        unsafe { Node::drop(tail, &self.allocator) };

        Some(value)
    }

    pub fn remove(&mut self, at: usize) -> Option<T> {
        if self.is_empty() || at >= self.len() {
            return None;
        }

        let mut cursor = self.cursor_mut();
        cursor.advance_forward_many(at);
        cursor.remove()
    }

    /// Create an iterator over a range of indices of which to be removed from
    /// the [`DoublyLinkedList`]. Returns an error if the range is out of bounds
    /// of the list length.
    pub fn drain<R: core::ops::RangeBounds<usize>>(&mut self, range: R) -> Result<Drain<'_, A, T>, DrainRangeError> {
        let len = self.len;
        let start = match range.start_bound() {
            Bound::Included(start) => *start,
            Bound::Excluded(start) => start.checked_add(1).ok_or(DrainRangeError)?,
            Bound::Unbounded => 0,
        };

        let end = match range.end_bound() {
            Bound::Included(end) => end.checked_add(1).ok_or(DrainRangeError)?,
            Bound::Excluded(end) => *end,
            Bound::Unbounded => len,
        };

        if start > end || end > len || start >= len {
            return Err(DrainRangeError);
        }

        let range = start..end;

        let mut cursor = self.cursor_mut();
        cursor.advance_forward_many(start);

        Ok(Drain(cursor, range))
    }

    pub fn front(&self) -> Option<&T> {
        let head_ptr = self.head?.as_ptr();
        Some(unsafe { &*addr_of!((*head_ptr).value) })
    }

    pub fn front_mut(&mut self) -> Option<&mut T> {
        let head_ptr = self.head?.as_ptr();
        Some(unsafe { &mut *addr_of_mut!((*head_ptr).value) })
    }

    pub fn back(&self) -> Option<&T> {
        let tail_ptr = self.tail?.as_ptr();
        Some(unsafe { &*addr_of!((*tail_ptr).value) })
    }

    pub fn back_mut(&mut self) -> Option<&mut T> {
        let tail_ptr = self.tail?.as_ptr();
        Some(unsafe { &mut *addr_of_mut!((*tail_ptr).value) })
    }

    pub fn at(&self, index: usize) -> Option<&T> {
        let node = self.node_at(index)?;
        let node_ptr = node.as_ptr();
        Some(unsafe { &*addr_of!((*node_ptr).value) })
    }

    pub fn at_mut(&self, index: usize) -> Option<&mut T> {
        let node = self.node_at(index)?;
        let node_ptr = node.as_ptr();
        Some(unsafe { &mut *addr_of_mut!((*node_ptr).value) })
    }

    pub fn cursor_mut(&mut self) -> CursorMut<'_, A, T> {
        let current = self.head;
        CursorMut { list: self, current, index: 0 }
    }

    pub fn cursor_mut_back(&mut self) -> CursorMut<'_, A, T> {
        let current = self.tail;
        let index = self.len.saturating_sub(1);
        CursorMut { list: self, current, index }
    }

    /// Create an [`Iterator`] over the shared references to the elements in
    /// this [`DoublyLinkedList`]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(self)
    }

    /// Create an [`Iterator`] over the unique references to the elements in
    /// this [`DoublyLinkedList`]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut::new(self)
    }

    fn node_at(&self, index: usize) -> Option<NonNull<Node<T>>> {
        let mut curr = self.head?;

        for _ in 0..index {
            let curr_ptr = curr.as_ptr();
            curr = unsafe { addr_of!((*curr_ptr).next).read()? };
        }

        Some(curr)
    }
}

impl<A: Allocator + Clone, T: Clone> DoublyLinkedList<A, T> {
    /// Clone this [`DoublyLinkedList`] in a fallible manner. This is a
    /// non-trait method version of [`Clone::clone`] that will not panic on
    /// allocation failure.
    pub fn fallible_clone(&self) -> Result<Self, AllocError> {
        let mut new_list = Self::new(self.allocator.clone());
        new_list.extend_from(self.iter().cloned())?;
        Ok(new_list)
    }
}

impl<A: Allocator + AllocatorCanMerge, T> DoublyLinkedList<A, T> {
    /// Append another [`DoublyLinkedList`] to this one, leaving it empty. Note:
    /// this method requires that the [`Allocator`] implements
    /// [`AllocatorCanMerge`], see the documentation for the trait for more
    /// information as to why.
    pub fn append(&mut self, other: &mut Self) {
        if self.head.is_none() {
            self.len = other.len;
            other.len = 0;
            self.head = other.head.take();
            self.tail = other.tail.take();
            return;
        }

        // No items to append if `other` is an empty list
        let Some(other_head) = other.head.take() else { return };
        other.tail.take();

        self.len += other.len;
        other.len = 0;

        let last_node = self.tail.unwrap();
        let last_node_ptr = last_node.as_ptr();
        unsafe { addr_of_mut!((*last_node_ptr).next).write(Some(other_head)) };
    }
}

impl<A: Allocator, T> Drop for DoublyLinkedList<A, T> {
    fn drop(&mut self) {
        #[allow(clippy::redundant_pattern_matching)]
        while let Some(_) = self.pop_front() {}
    }
}

impl<A1: Allocator, A2: Allocator, T: PartialEq> PartialEq<DoublyLinkedList<A2, T>> for DoublyLinkedList<A1, T> {
    fn eq(&self, other: &DoublyLinkedList<A2, T>) -> bool {
        self.iter().zip(other).all(|(a, b)| a == b)
    }
}

impl<A: Allocator, T: core::fmt::Debug> core::fmt::Debug for DoublyLinkedList<A, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut list = f.debug_list();

        for item in self {
            list.entry(item);
        }

        list.finish()
    }
}

impl<A: Allocator + Clone, T: Clone> Clone for DoublyLinkedList<A, T> {
    fn clone(&self) -> Self {
        self.fallible_clone().unwrap()
    }
}

impl<'a, A: Allocator, T> IntoIterator for &'a DoublyLinkedList<A, T> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, A: Allocator, T> IntoIterator for &'a mut DoublyLinkedList<A, T> {
    type IntoIter = IterMut<'a, T>;
    type Item = &'a mut T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// An [`Iterator`] over shared references to the elements in a [`DoublyLinkedList`]
pub struct Iter<'a, T: 'a>(Option<NonNull<Node<T>>>, core::marker::PhantomData<&'a ()>);

impl<'a, T: 'a> Iter<'a, T> {
    /// Create a new [`Iter`] over the elements in the given
    /// [`DoublyLinkedList`]
    pub fn new<A: Allocator>(linked_list: &'a DoublyLinkedList<A, T>) -> Self {
        Self(linked_list.head, core::marker::PhantomData)
    }
}

impl<'a, T: 'a> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.0?;
        let node_ptr = node.as_ptr();
        let next = unsafe { addr_of!((*node_ptr).next).read() };
        self.0 = next;

        Some(unsafe { &*addr_of!((*node_ptr).value) })
    }
}

impl<'a, T: 'a> FusedIterator for Iter<'a, T> {}

/// An [`Iterator`] over unique references to the elements in a [`DoublyLinkedList`]
pub struct IterMut<'a, T: 'a>(Option<NonNull<Node<T>>>, core::marker::PhantomData<&'a mut ()>);

impl<'a, T: 'a> IterMut<'a, T> {
    /// Create a new [`Iter`] over the elements in the given
    /// [`DoublyLinkedList`]
    pub fn new<A: Allocator>(linked_list: &'a mut DoublyLinkedList<A, T>) -> Self {
        Self(linked_list.head, core::marker::PhantomData)
    }
}

impl<'a, T: 'a> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.0?;
        let node_ptr = node.as_ptr();
        let next = unsafe { addr_of!((*node_ptr).next).read() };
        self.0 = next;

        Some(unsafe { &mut *addr_of_mut!((*node_ptr).value) })
    }
}

impl<'a, T: 'a> FusedIterator for IterMut<'a, T> {}

pub struct CursorMut<'a, A: Allocator, T> {
    list: &'a mut DoublyLinkedList<A, T>,
    current: Option<NonNull<Node<T>>>,
    index: usize,
}

impl<'a, A: Allocator, T> CursorMut<'a, A, T> {
    pub fn advance_forward(&mut self) {
        let Some(current) = self.current else { return };
        let current_ptr = current.as_ptr();

        self.index += 1;
        self.current = unsafe { addr_of!((*current_ptr).next).read() };
    }

    /// Advance the cursor forward `n` nodes
    pub fn advance_forward_many(&mut self, n: usize) {
        for _ in 0..n.min(self.list.len()) {
            self.advance_forward();
        }
    }

    pub fn advance_backward(&mut self) {
        if self.index == self.list.len() {
            self.index = self.index.saturating_sub(1);
            self.current = self.list.tail;
            return;
        }

        let Some(current) = self.current else { return };
        let current_ptr = current.as_ptr();

        if let Some(prev) = unsafe { addr_of!((*current_ptr).prev).read() } {
            self.index -= 1;
            self.current = Some(prev);
        }
    }

    /// Advance the cursor forward `n` nodes
    pub fn advance_backward_many(&mut self, n: usize) {
        for _ in 0..n.min(self.list.len()) {
            self.advance_backward();
        }
    }

    pub fn current(&self) -> Option<&T> {
        let current_ptr = self.current?.as_ptr();
        Some(unsafe { &*addr_of!((*current_ptr).value) })
    }

    pub fn current_mut(&mut self) -> Option<&mut T> {
        let current_ptr = self.current?.as_ptr();
        Some(unsafe { &mut *addr_of_mut!((*current_ptr).value) })
    }

    pub fn previous(&self) -> Option<&T> {
        let Some(current) = self.current else { return Some(unsafe { &(*self.list.tail?.as_ptr()).value }) };
        let current_ptr = current.as_ptr();

        match unsafe { addr_of!((*current_ptr).prev).read() } {
            Some(prev) => {
                let prev_ptr = prev.as_ptr();
                Some(unsafe { &*addr_of!((*prev_ptr).value) })
            }
            None => None,
        }
    }

    pub fn previous_mut(&mut self) -> Option<&mut T> {
        let Some(current) = self.current else { return Some(unsafe { &mut (*self.list.tail?.as_ptr()).value }) };
        let current_ptr = current.as_ptr();

        match unsafe { addr_of!((*current_ptr).prev).read() } {
            Some(prev) => {
                let prev_ptr = prev.as_ptr();
                Some(unsafe { &mut *addr_of_mut!((*prev_ptr).value) })
            }
            None => None,
        }
    }

    pub fn next(&self) -> Option<&T> {
        let current = self.current?;
        let current_ptr = current.as_ptr();

        match unsafe { addr_of!((*current_ptr).next).read() } {
            Some(next) => {
                let next_ptr = next.as_ptr();
                Some(unsafe { &*addr_of!((*next_ptr).value) })
            }
            None => None,
        }
    }

    pub fn next_mut(&mut self) -> Option<&mut T> {
        let current = self.current?;
        let current_ptr = current.as_ptr();

        match unsafe { addr_of!((*current_ptr).next).read() } {
            Some(next) => {
                let next_ptr = next.as_ptr();
                Some(unsafe { &mut *addr_of_mut!((*next_ptr).value) })
            }
            None => None,
        }
    }

    pub fn insert(&mut self, value: T) -> Result<(), AllocError> {
        match self.current {
            // We're an empty list without a head
            None if self.index == 0 => {
                self.list.push_front(value)?;
                self.current = self.list.head;
                Ok(())
            }
            // We're at the "ghost" tail element
            None => {
                self.list.push_back(value)?;
                self.current = self.list.tail;
                Ok(())
            }
            Some(current) => {
                if self.index == 0 {
                    self.list.push_front(value)?;
                    self.current = self.list.head;
                    return Ok(());
                }

                let current_ptr = current.as_ptr();
                let prev = unsafe { addr_of!((*current_ptr).prev).read() };

                let new_current = Node::new(&self.list.allocator, value, prev, self.current)?;

                self.current = Some(new_current);
                self.list.len += 1;

                Ok(())
            }
        }
    }

    /// Remove the current element from the [`DoublyLinkedList`], returning it's value, if it
    /// exists. This will also advance the cursor forward to the next element,
    /// if there is one.
    pub fn remove(&mut self) -> Option<T> {
        let current = self.current?;
        let current_ptr = current.as_ptr();

        let current_node = unsafe { current_ptr.read() };
        let maybe_next = current_node.next;

        if let Some(next) = maybe_next {
            let next_ptr = next.as_ptr();
            unsafe { addr_of_mut!((*next_ptr).prev).write(current_node.prev) };
        }

        match current_node.prev {
            Some(prev) => {
                let prev_ptr = prev.as_ptr();
                unsafe { addr_of_mut!((*prev_ptr).next).write(maybe_next) };
            }
            None => self.list.head = maybe_next,
        }

        unsafe { Node::drop(current, &self.list.allocator) };
        self.list.len -= 1;
        self.current = maybe_next;

        Some(current_node.value)
    }
}

pub struct Drain<'a, A: Allocator, T>(CursorMut<'a, A, T>, Range<usize>);

impl<'a, A: Allocator, T> Iterator for Drain<'a, A, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.1.next()?;
        self.0.remove()
    }
}

impl<'a, A: Allocator, T> Drop for Drain<'a, A, T> {
    fn drop(&mut self) {
        for _ in self.by_ref() {}
    }
}

#[derive(Debug)]
struct Node<T> {
    value: T,
    prev: Option<NonNull<Self>>,
    next: Option<NonNull<Self>>,
}

impl<T> Node<T> {
    fn new<A: Allocator>(
        allocator: &A,
        value: T,
        prev: Option<NonNull<Self>>,
        next: Option<NonNull<Self>>,
    ) -> Result<NonNull<Self>, AllocError> {
        let me: NonNull<Self> = allocator.allocate(Layout::new::<Self>())?.cast();
        unsafe { me.as_ptr().write(Self { value, prev, next }) };
        if let Some(prev) = prev {
            let prev_ptr = prev.as_ptr();
            unsafe { addr_of_mut!((*prev_ptr).next).write(Some(me)) };
        }

        if let Some(next) = next {
            let next_ptr = next.as_ptr();
            unsafe { addr_of_mut!((*next_ptr).prev).write(Some(me)) };
        }

        Ok(me)
    }

    unsafe fn drop<A: Allocator>(me: NonNull<Self>, allocator: &A) {
        unsafe { allocator.deallocate(me.cast(), Layout::new::<Self>()) };
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{alloc::Global, string::String};

    #[test]
    fn push_insert() {
        let mut ll = DoublyLinkedList::new(Global);
        ll.push_front(String::from("first")).unwrap();
        assert_eq!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert_eq!(ll.front().unwrap(), "first");
        assert_eq!(ll.back().unwrap(), "first");
        assert!(ll.at(1).is_none());

        ll.push_back(String::from("second")).unwrap();
        assert_ne!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert_eq!(ll.at(1).unwrap(), "second");
        assert_eq!(ll.front().unwrap(), "first");
        assert_eq!(ll.back().unwrap(), "second");
        assert!(ll.at(2).is_none());

        let mut ll = DoublyLinkedList::new(Global);
        ll.insert(0, String::from("first")).unwrap();
        assert_eq!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert!(ll.at(1).is_none());

        ll.insert(1, String::from("second")).unwrap();
        assert_ne!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert_eq!(ll.at(1).unwrap(), "second");
        assert!(ll.at(2).is_none());

        let mut ll = DoublyLinkedList::new(Global);
        assert!(ll.insert(1, String::from("first")).is_err());
        assert!(ll.insert(2, String::from("first")).is_err());
        assert!(ll.at(0).is_none());
        assert!(ll.at(1).is_none());
        ll.insert(0, String::from("first")).unwrap();
        ll.insert(1, String::from("second")).unwrap();
        assert_ne!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert_eq!(ll.at(1).unwrap(), "second");

        let mut ll = DoublyLinkedList::new(Global);
        ll.push_back(String::from("first")).unwrap();
        assert_eq!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert!(ll.at(1).is_none());
    }

    #[test]
    fn pop_front_bug() {
        let mut ll = DoublyLinkedList::new(Global);
        ll.push_back(String::from("first")).unwrap();
        ll.push_back(String::from("second")).unwrap();
        ll.push_back(String::from("third")).unwrap();
        assert_eq!(ll.remove(2).unwrap(), "third");
        assert_eq!(ll.remove(1).unwrap(), "second");
        assert_eq!(ll.remove(0).unwrap(), "first");
    }

    #[test]
    fn pop_remove() {
        let mut ll = DoublyLinkedList::new(Global);
        ll.push_front(String::from("first")).unwrap();
        ll.push_front(String::from("second")).unwrap();
        assert_eq!(ll.len(), 2);
        assert_eq!(ll.pop_front().unwrap(), "second");
        assert_eq!(ll.len(), 1);
        assert_eq!(ll.pop_front().unwrap(), "first");
        assert_eq!(ll.len(), 0);

        ll.push_front(String::from("first")).unwrap();
        ll.push_back(String::from("second")).unwrap();
        assert_eq!(ll.len(), 2);
        assert_eq!(ll.pop_back().unwrap(), "second");
        assert_eq!(ll.len(), 1);
        assert_eq!(ll.pop_back().unwrap(), "first");
        assert_eq!(ll.len(), 0);

        let mut ll = DoublyLinkedList::new(Global);
        ll.insert(0, String::from("first")).unwrap();
        assert_eq!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert!(ll.at(1).is_none());

        ll.insert(1, String::from("second")).unwrap();
        assert_ne!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert_eq!(ll.at(1).unwrap(), "second");
        assert!(ll.at(2).is_none());

        ll.insert(1, String::from("third")).unwrap();
        assert_ne!(ll.head, ll.tail);
        assert_eq!(ll.at(0).unwrap(), "first");
        assert_eq!(ll.at(1).unwrap(), "third");
        assert_eq!(ll.at_mut(2).unwrap(), "second");
        assert!(ll.at(3).is_none());
        assert_eq!(ll.remove(2).unwrap(), "second");
        assert_eq!(ll.remove(1).unwrap(), "third");
        assert_eq!(ll.remove(0).unwrap(), "first");

        let mut ll: DoublyLinkedList<Global, String> = DoublyLinkedList::new(Global);
        assert!(ll.pop_front().is_none());
        assert!(ll.pop_back().is_none());
        assert!(ll.remove(0).is_none());
        assert!(ll.remove(1).is_none());

        let mut ll = DoublyLinkedList::new(Global);
        ll.push_back(String::from("first")).unwrap();
        ll.push_back(String::from("second")).unwrap();
        ll.push_back(String::from("third")).unwrap();
        ll.push_back(String::from("forth")).unwrap();
        ll.push_back(String::from("fifth")).unwrap();

        assert_eq!(ll.front_mut().unwrap(), "first");
        assert_eq!(ll.back_mut().unwrap(), "fifth");

        assert_eq!(ll.remove(3).unwrap(), "forth");
        assert_eq!(ll.remove(2).unwrap(), "third");
        assert_eq!(ll.remove(0).unwrap(), "first");
        assert_eq!(ll.remove(0).unwrap(), "second");
        assert_eq!(ll.remove(0).unwrap(), "fifth");
    }

    #[test]
    fn cursor_mut() {
        let mut ll = DoublyLinkedList::new(Global);
        let mut cursor = ll.cursor_mut();
        cursor.insert(String::from("first")).unwrap();
        cursor.insert(String::from("second")).unwrap();
        assert!(cursor.previous().is_none());
        assert_eq!(cursor.current().unwrap(), "second");
        assert_eq!(cursor.next().unwrap(), "first");
        cursor.advance_forward();

        cursor.advance_backward();
        cursor.advance_forward();

        cursor.advance_forward();
        cursor.insert(String::from("third")).unwrap();

        assert_eq!(cursor.previous().unwrap(), "first");
        assert_eq!(ll.back().unwrap(), "third");

        let mut ll = DoublyLinkedList::new(Global);
        let mut cursor = ll.cursor_mut();
        cursor.advance_backward();
        cursor.advance_forward();
        cursor.advance_backward();
        assert!(cursor.previous().is_none());
        assert!(cursor.current().is_none());
        assert!(cursor.next().is_none());

        cursor.insert(String::from("aaa1")).unwrap();
        assert_eq!(cursor.current().unwrap(), "aaa1");
        assert!(cursor.previous().is_none());

        cursor.insert(String::from("bbb2")).unwrap();
        assert_eq!(cursor.current().unwrap(), "bbb2");
        assert_eq!(cursor.next().unwrap(), "aaa1");
        assert!(cursor.previous().is_none());

        cursor.advance_forward();
        assert_eq!(cursor.current().unwrap(), "aaa1");
        assert_eq!(cursor.previous().unwrap(), "bbb2");
        assert!(cursor.next().is_none());

        cursor.insert(String::from("ccc3")).unwrap();
        assert_eq!(cursor.next().unwrap(), "aaa1");
        assert_eq!(cursor.previous().unwrap(), "bbb2");

        cursor.insert(String::from("ddd4")).unwrap();
        assert_eq!(cursor.current().unwrap(), "ddd4");
        assert_eq!(cursor.next().unwrap(), "ccc3");
        assert_eq!(cursor.previous().unwrap(), "bbb2");
    }

    #[test]
    fn cursor_mut_back() {
        let mut ll = DoublyLinkedList::new(Global);
        ll.push_back(String::from("aaa1")).unwrap();
        ll.push_back(String::from("bbb2")).unwrap();

        let mut cursor = ll.cursor_mut_back();
        assert_eq!(cursor.current_mut().unwrap(), "bbb2");
        assert_eq!(cursor.previous_mut().unwrap(), "aaa1");
        assert!(cursor.next_mut().is_none());
        assert!(cursor.next().is_none());

        cursor.advance_backward();
        cursor.advance_backward();
        assert_eq!(cursor.current_mut().unwrap(), "aaa1");
        assert_eq!(cursor.next_mut().unwrap(), "bbb2");
        assert_eq!(cursor.next().unwrap(), "bbb2");
        assert!(cursor.previous_mut().is_none());

        let mut ll = DoublyLinkedList::new(Global);
        ll.push_front(String::from("aaa1")).unwrap();
        let mut cursor = ll.cursor_mut_back();
        cursor.advance_forward();
        cursor.insert(String::from("bbb2")).unwrap();
        assert_eq!(ll.back().unwrap(), "bbb2");
    }
}
