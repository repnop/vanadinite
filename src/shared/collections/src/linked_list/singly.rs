// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{
    alloc::{AllocError, Allocator, Layout},
    iter::FusedIterator,
    ops::Bound,
    ptr::{addr_of, addr_of_mut, NonNull},
};

use super::AllocatorCanMerge;

/// An error that occurred during a [`SinglyLinkedList::insert`] operation
#[derive(Debug)]
pub enum InsertError<T> {
    /// The position of the insert was not within the length of the list, the
    /// would-be-inserted value is returned
    Position(T),
    /// The allocator was unable to allocate enough memory for a new linked list
    /// entry
    AllocError(AllocError),
}

impl<T> From<AllocError> for InsertError<T> {
    fn from(v: AllocError) -> Self {
        Self::AllocError(v)
    }
}

/// The range supplied to [`SinglyLinkedList::drain`] was not valid for the
/// given [`SinglyLinkedList`]
#[derive(Debug, Clone, Copy)]
pub struct DrainRangeError;

/// An intrusive singly linked list
pub struct SinglyLinkedList<A: Allocator, T> {
    head: Option<NonNull<Node<T>>>,
    len: usize,
    allocator: A,
}

impl<A: Allocator, T> SinglyLinkedList<A, T> {
    /// Create a new [`SinglyLinkedList`]
    pub const fn new(allocator: A) -> Self {
        Self { head: None, len: 0, allocator }
    }

    /// The number of elements in this [`SinglyLinkedList`]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether or not this [`SinglyLinkedList`] is empty
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Push a value to the front (head) of the linked list
    pub fn push_front(&mut self, value: T) -> Result<(), AllocError> {
        let new_node = Node::new(&self.allocator, value)?;

        match self.head {
            Some(head) => {
                let ptr = new_node.as_ptr();
                unsafe { addr_of_mut!((*ptr).next).write(Some(head)) };
                self.head = Some(new_node);
            }
            None => self.head = Some(new_node),
        }

        self.len += 1;
        Ok(())
    }

    /// Append a new value to the end (tail) of the linked list
    pub fn push_back(&mut self, value: T) -> Result<(), AllocError> {
        let new_node = Node::new(&self.allocator, value)?;
        let node = self.last_node();

        match node {
            None => self.head = Some(new_node),
            Some(node) => {
                let ptr = node.as_ptr();
                unsafe { addr_of_mut!((*ptr).next).write(Some(new_node)) };
            }
        }

        self.len += 1;
        Ok(())
    }

    /// Pop the front (head) of the linked list, if one exists, returning the
    /// contained value
    pub fn pop_front(&mut self) -> Option<T> {
        let head_ptr = self.head?;
        let head = unsafe { head_ptr.as_ptr().read() };

        match head.next {
            Some(new_head) => self.head = Some(new_head),
            None => self.head = None,
        }

        unsafe { Node::drop(&self.allocator, head_ptr) };
        self.len -= 1;
        Some(head.value)
    }

    /// Pop the end (tail) of the linked list, if one exists, returning the
    /// contained value
    pub fn pop_back(&mut self) -> Option<T> {
        let mut current_node = self.head?;
        let mut previous_node = None;

        loop {
            let ptr = current_node.as_ptr();
            match unsafe { addr_of!((*ptr).next).read() } {
                Some(next) => {
                    previous_node = Some(current_node);
                    current_node = next;
                }
                None => {
                    match previous_node {
                        Some(prev) => {
                            let prev_ptr = prev.as_ptr();
                            unsafe { addr_of_mut!((*prev_ptr).next).write(None) };
                        }
                        // None here means that we have a list with only the head
                        // node
                        None => self.head = None,
                    }

                    let current_node_value = unsafe { current_node.as_ptr().read() };
                    unsafe { Node::drop(&self.allocator, current_node) };
                    self.len -= 1;
                    return Some(current_node_value.value);
                }
            }
        }
    }

    /// Extend the [`SinglyLinkedList`] from an iterator. This is a non-trait
    /// method version of the [`Extend::extend`] method that will not panic on
    /// allocation failure.
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) -> Result<(), AllocError> {
        let mut iterator = iter.into_iter();
        let mut last_node = match self.last_node() {
            Some(node) => node,
            None => {
                let Some(value) = iterator.next() else { return Ok(()) };
                self.push_back(value)?;
                self.head.unwrap()
            }
        };

        for value in iterator {
            let new_node = Node::new(&self.allocator, value)?;
            let last_node_ptr = last_node.as_ptr();
            unsafe { addr_of_mut!((*last_node_ptr).next).write(Some(new_node)) };
            last_node = new_node;
            self.len += 1;
        }

        Ok(())
    }

    /// Insert a new value at the given position in the linked list.
    /// `list.insert(0, value)` is equivalent to `list.push_front(value)`. If
    /// `at` does not correspond to a position within the linked list,
    /// `Err(InsertError::Position(T))` will be returned, giving back ownership of
    /// the provided value.
    pub fn insert(&mut self, at: usize, value: T) -> Result<(), InsertError<T>> {
        if at > self.len {
            return Err(InsertError::Position(value));
        }

        let mut cursor = self.cursor_mut();
        cursor.advance_many(at);
        cursor.insert(value).map_err(InsertError::AllocError)
    }

    /// Get a shared reference to the value at the given position in the list,
    /// if it exists
    pub fn get(&self, at: usize) -> Option<&T> {
        let mut current_node = self.head?;
        let mut position = 0;

        while position < at {
            let node_ptr = current_node.as_ptr();
            match unsafe { addr_of!((*node_ptr).next).read() } {
                Some(next) => current_node = next,
                None => return None,
            }

            position += 1;
        }

        let current_node_ptr = current_node.as_ptr();
        Some(unsafe { &*addr_of!((*current_node_ptr).value) })
    }

    /// Get a shared reference to the last value in the linked list, if it
    /// exists
    pub fn front(&self) -> Option<&T> {
        let node = self.head?.as_ptr();
        Some(unsafe { &*addr_of!((*node).value) })
    }

    /// Get a unique reference to the last value in the linked list, if it
    /// exists
    pub fn front_mut(&mut self) -> Option<&mut T> {
        let node = self.head?.as_ptr();
        Some(unsafe { &mut *addr_of_mut!((*node).value) })
    }

    /// Get a shared reference to the last value in the linked list, if it
    /// exists
    pub fn back(&self) -> Option<&T> {
        let node = self.last_node()?.as_ptr();
        Some(unsafe { &*addr_of!((*node).value) })
    }

    /// Get a unique reference to the last value in the linked list, if it
    /// exists
    pub fn back_mut(&mut self) -> Option<&mut T> {
        let node = self.last_node()?.as_ptr();
        Some(unsafe { &mut *addr_of_mut!((*node).value) })
    }

    /// Create an [`Iterator`] over the shared references to the elements in
    /// this [`SinglyLinkedList`]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(self)
    }

    /// Create an [`Iterator`] over the unique references to the elements in
    /// this [`SinglyLinkedList`]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut::new(self)
    }

    /// Create a cursor over the elements in this [`SinglyLinkedList`]
    pub fn cursor(&self) -> Cursor<'_, T> {
        Cursor { list: core::marker::PhantomData, position: self.head }
    }

    /// Create a cursor over the elements in this [`SinglyLinkedList`] which can
    /// also modify the structure and elements
    pub fn cursor_mut(&mut self) -> CursorMut<'_, A, T> {
        let position = self.head;
        CursorMut { list: self, position, previous: None, index: 0 }
    }

    /// Create an iterator over a range of indices of which to be removed from
    /// the [`SinglyLinkedList`]. Returns an error if the range is out of bounds
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
        cursor.advance_many(start);

        Ok(Drain(cursor, range))
    }

    /// Create a new [`SinglyLinkedList`] from an iterator
    pub fn from_iter<I: IntoIterator<Item = T>>(allocator: A, iter: I) -> Result<Self, AllocError> {
        let mut me = Self::new(allocator);
        me.extend_from(iter)?;

        Ok(me)
    }

    fn last_node(&self) -> Option<NonNull<Node<T>>> {
        let mut current_node = self.head?;

        loop {
            let ptr = current_node.as_ptr();
            match unsafe { addr_of!((*ptr).next).read() } {
                Some(next) => current_node = next,
                None => return Some(current_node),
            }
        }
    }
}

impl<A: Allocator + AllocatorCanMerge, T> SinglyLinkedList<A, T> {
    /// Append another [`SinglyLinkedList`] to this one, leaving it empty. Note:
    /// this method requires that the [`Allocator`] implements
    /// [`AllocatorCanMerge`], see the documentation for the trait for more
    /// information as to why.
    pub fn append(&mut self, other: &mut Self) {
        if self.head.is_none() {
            self.len = other.len;
            other.len = 0;
            return self.head = other.head.take();
        }

        // No items to append if `other` is an empty list
        let Some(other_head) = other.head.take() else { return };

        self.len += other.len;
        other.len = 0;

        let last_node = self.last_node().unwrap();
        let last_node_ptr = last_node.as_ptr();
        unsafe { addr_of_mut!((*last_node_ptr).next).write(Some(other_head)) };
    }
}

impl<A: Allocator + Clone, T: Clone> SinglyLinkedList<A, T> {
    /// Clone this [`SinglyLinkedList`] in a fallible manner. This is a
    /// non-trait method version of [`Clone::clone`] that will not panic on
    /// allocation failure.
    pub fn fallible_clone(&self) -> Result<Self, AllocError> {
        let mut new_list = Self::new(self.allocator.clone());
        new_list.extend_from(self.iter().cloned())?;
        Ok(new_list)
    }
}

impl<A: Allocator + Clone, T: Clone> Clone for SinglyLinkedList<A, T> {
    fn clone(&self) -> Self {
        self.fallible_clone().unwrap()
    }
}

impl<'a, A: Allocator, T> IntoIterator for &'a SinglyLinkedList<A, T> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, A: Allocator, T> IntoIterator for &'a mut SinglyLinkedList<A, T> {
    type IntoIter = IterMut<'a, T>;
    type Item = &'a mut T;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<A1: Allocator, A2: Allocator, T: PartialEq> PartialEq<SinglyLinkedList<A2, T>> for SinglyLinkedList<A1, T> {
    fn eq(&self, other: &SinglyLinkedList<A2, T>) -> bool {
        self.iter().zip(other).all(|(a, b)| a == b)
    }
}

impl<A: Allocator, T: core::fmt::Debug> core::fmt::Debug for SinglyLinkedList<A, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut list = f.debug_list();

        for item in self {
            list.entry(item);
        }

        list.finish()
    }
}

impl<A: Allocator, T> Drop for SinglyLinkedList<A, T> {
    fn drop(&mut self) {
        #[allow(clippy::redundant_pattern_matching)]
        while let Some(_) = self.pop_front() {}
    }
}

/// An [`Iterator`] over shared references to the elements in a [`SinglyLinkedList`]
pub struct Iter<'a, T: 'a>(Option<NonNull<Node<T>>>, core::marker::PhantomData<&'a ()>);

impl<'a, T: 'a> Iter<'a, T> {
    /// Create a new [`Iter`] over the elements in the given
    /// [`SinglyLinkedList`]
    pub fn new<A: Allocator>(linked_list: &'a SinglyLinkedList<A, T>) -> Self {
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

/// An [`Iterator`] over unique references to the elements in a [`SinglyLinkedList`]
pub struct IterMut<'a, T: 'a>(Option<NonNull<Node<T>>>, core::marker::PhantomData<&'a mut ()>);

impl<'a, T: 'a> IterMut<'a, T> {
    /// Create a new [`Iter`] over the elements in the given
    /// [`SinglyLinkedList`]
    pub fn new<A: Allocator>(linked_list: &'a mut SinglyLinkedList<A, T>) -> Self {
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

/// See [`SinglyLinkedList::drain`]
pub struct Drain<'a, A: Allocator, T>(CursorMut<'a, A, T>, core::ops::Range<usize>);

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

/// A cursor over a shared reference to a [`SinglyLinkedList`] which can
/// traverse the list of elements
pub struct Cursor<'a, T> {
    list: core::marker::PhantomData<&'a ()>,
    position: Option<NonNull<Node<T>>>,
}

impl<'a, T> Cursor<'a, T> {
    /// Advance the cursor position
    pub fn advance(&mut self) {
        let Some(position_ptr) = self.position.map(|p| p.as_ptr()) else { return };
        self.position = unsafe { addr_of!((*position_ptr).next).read() };
    }

    /// Retrieve a shared reference to the current position's value, if it
    /// exists
    pub fn current(&self) -> Option<&'a T> {
        let position_ptr = self.position?.as_ptr();
        Some(unsafe { &*addr_of!((*position_ptr).value) })
    }

    /// Peek at the next position's value, if it exists
    pub fn next(&self) -> Option<&'a T> {
        let position_ptr = self.position?.as_ptr();
        let next_ptr = unsafe { addr_of!((*position_ptr).next).read() }?.as_ptr();
        Some(unsafe { &*addr_of!((*next_ptr).value) })
    }
}

/// A cursor over a unique reference to a [`SinglyLinkedList`] which can
/// traverse and modify the linked list's structure
pub struct CursorMut<'a, A: Allocator, T> {
    list: &'a mut SinglyLinkedList<A, T>,
    index: usize,
    previous: Option<NonNull<Node<T>>>,
    position: Option<NonNull<Node<T>>>,
}

impl<'a, A: Allocator, T> CursorMut<'a, A, T> {
    /// Advance the cursor position to the next element in the list
    pub fn advance(&mut self) {
        let Some(position_ptr) = self.position.map(|p| p.as_ptr()) else { return };
        self.index += 1;
        self.previous = self.position;
        self.position = unsafe { addr_of!((*position_ptr).next).read() };
    }

    /// Advance the cursor position to the next element in the list
    pub fn advance_many(&mut self, n: usize) {
        for _ in 0..n.min(self.list.len) {
            self.advance();
        }
    }

    /// Retrieve a shared reference to the current position's value, if it
    /// exists
    pub fn current(&self) -> Option<&'a T> {
        let position_ptr = self.position?.as_ptr();
        Some(unsafe { &*addr_of!((*position_ptr).value) })
    }

    /// Retrieve a unique reference to the current position's value, if it
    /// exists
    pub fn current_mut(&mut self) -> Option<&'a mut T> {
        let position_ptr = self.position?.as_ptr();
        Some(unsafe { &mut *addr_of_mut!((*position_ptr).value) })
    }

    /// Peek at the next position's value, if it exists
    pub fn next(&self) -> Option<&'a T> {
        let position_ptr = self.position?.as_ptr();
        let next_ptr = unsafe { addr_of!((*position_ptr).next).read() }?.as_ptr();
        Some(unsafe { &*addr_of!((*next_ptr).value) })
    }

    /// Peek at the next position's value, if it exists
    pub fn next_mut(&mut self) -> Option<&'a mut T> {
        let position_ptr = self.position?.as_ptr();
        let next_ptr = unsafe { addr_of!((*position_ptr).next).read() }?.as_ptr();
        Some(unsafe { &mut *addr_of_mut!((*next_ptr).value) })
    }

    /// Remove the current element from the [`SinglyLinkedList`], returning it's value, if it
    /// exists. This will also advance the cursor forward to the next element,
    /// if there is one.
    pub fn remove(&mut self) -> Option<T> {
        let current = self.position?;
        let current_ptr = current.as_ptr();
        let maybe_next = unsafe { addr_of!((*current_ptr).next).read() };

        match self.previous {
            Some(prev) => {
                let prev_ptr = prev.as_ptr();
                unsafe { addr_of_mut!((*prev_ptr).next).write(maybe_next) };
            }
            None => self.list.head = maybe_next,
        }

        let value = unsafe { addr_of!((*current_ptr).value).read() };
        unsafe { Node::drop(&self.list.allocator, current) };
        self.list.len -= 1;
        self.position = maybe_next;

        Some(value)
    }

    /// Insert a new value at the current position, making it the pointed-to
    /// element. An unadvanced cursor insert is equivalent to [`SinglyLinkedList::push_front`], and
    pub fn insert(&mut self, value: T) -> Result<(), AllocError> {
        match (self.previous, self.position) {
            (Some(prev), Some(current)) => {
                let prev_ptr = prev.as_ptr();

                let new_current = Node::new(&self.list.allocator, value)?;
                let new_current_ptr = new_current.as_ptr();

                unsafe { addr_of_mut!((*prev_ptr).next).write(Some(new_current)) };
                unsafe { addr_of_mut!((*new_current_ptr).next).write(Some(current)) };

                self.list.len += 1;
                self.position = Some(new_current);

                Ok(())
            }
            (None, Some(_)) | (None, None) => {
                self.list.push_front(value)?;
                self.position = self.list.head;

                Ok(())
            }
            (Some(prev), None) => {
                let prev_ptr = prev.as_ptr();
                let new_current = Node::new(&self.list.allocator, value)?;

                unsafe { addr_of_mut!((*prev_ptr).next).write(Some(new_current)) };
                self.position = Some(new_current);

                self.list.len += 1;

                Ok(())
            }
        }
    }
}

impl<'a, A: Allocator + Clone, T> CursorMut<'a, A, T> {
    /// Split the [`SinglyLinkedList`] at the current element, returning a new
    /// [`SinglyLinkedList`] containing the current element and any elements
    /// following it. If the current [`SinglyLinkedList`] is empty, or the
    /// cursor has advanced to the end of the list, this method returns an empty
    /// [`SinglyLinkedList`].
    pub fn split(&mut self) -> SinglyLinkedList<A, T> {
        let current = self.position;
        match self.previous {
            Some(prev) => {
                let prev_ptr = prev.as_ptr();
                unsafe { addr_of_mut!((*prev_ptr).next).write(None) };

                let new_list_len = self.list.len - self.index;
                self.list.len -= new_list_len;
                self.position = self.previous;
                SinglyLinkedList { head: current, len: new_list_len, allocator: self.list.allocator.clone() }
            }
            None => {
                let head = self.list.head.take();
                let len = self.list.len;
                self.list.len = 0;
                self.position = None;

                SinglyLinkedList { head, len, allocator: self.list.allocator.clone() }
            }
        }
    }
}

impl<'a, A: Allocator + AllocatorCanMerge, T> CursorMut<'a, A, T> {
    /// Insert another [`SinglyLinkedList`] into the current position leaving
    /// `other` empty, and placing the current element and any following elements
    /// at the end
    pub fn insert_list(&mut self, other: &mut SinglyLinkedList<A, T>) {
        let Some(other_last_node) = other.last_node() else { return };
        let other_last_node_ptr = other_last_node.as_ptr();
        let head = other.head.take().unwrap();

        match self.previous {
            Some(previous) => {
                let prev_ptr = previous.as_ptr();
                unsafe { addr_of_mut!((*prev_ptr).next).write(Some(head)) };
                unsafe { addr_of_mut!((*other_last_node_ptr).next).write(self.position) };
                self.position = Some(head);
            }
            None => {
                let current_head = self.list.head;
                unsafe { addr_of_mut!((*other_last_node_ptr).next).write(current_head) };
                self.list.head = Some(head);
                self.position = Some(head);
            }
        }

        self.list.len += other.len;
        other.len = 0;
    }
}

struct Node<T> {
    value: T,
    next: Option<NonNull<Self>>,
}

impl<T> Node<T> {
    fn new<A: Allocator>(allocator: &A, value: T) -> Result<NonNull<Self>, AllocError> {
        let me: NonNull<Node<T>> = allocator.allocate(Layout::new::<Self>())?.as_non_null_ptr().cast();
        unsafe { me.as_ptr().write(Self { value, next: None }) };
        Ok(me)
    }

    unsafe fn drop<A: Allocator>(allocator: &A, me: NonNull<Self>) {
        unsafe { allocator.deallocate(me.cast(), Layout::new::<Self>()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;
    use std::string::String;

    #[test]
    fn push_pop() {
        let mut list = SinglyLinkedList::new(Global);
        list.push_front(String::from("test1")).unwrap();
        list.push_back(String::from("test2")).unwrap();
        list.push_front(String::from("test3")).unwrap();

        assert_eq!(list.pop_front(), Some(String::from("test3")));
        assert_eq!(list.pop_front(), Some(String::from("test1")));
        assert_eq!(list.pop_front(), Some(String::from("test2")));

        let mut list = SinglyLinkedList::new(Global);
        list.push_front(String::from("test1")).unwrap();
        list.push_back(String::from("test2")).unwrap();
        list.push_front(String::from("test3")).unwrap();

        assert_eq!(list.pop_back(), Some(String::from("test2")));
        assert_eq!(list.pop_back(), Some(String::from("test1")));
        assert_eq!(list.pop_back(), Some(String::from("test3")));
    }

    #[test]
    fn extend() {
        let mut list = SinglyLinkedList::new(Global);
        let iter = (0..10).map(|i| std::format!("{i}"));
        let iter2 = (10..12).map(|i| std::format!("{i}"));

        list.extend_from(iter.clone()).unwrap();
        list.extend_from(iter2.clone()).unwrap();

        for s in iter.chain(iter2) {
            assert_eq!(list.pop_front(), Some(s));
        }

        assert!(list.pop_front().is_none());
    }

    #[test]
    fn iter_append() {
        let mut list = SinglyLinkedList::new(Global);
        let iter = (0..10).map(|i| std::format!("{i}"));
        list.extend_from(iter.clone()).unwrap();
        let mut list_clone = list.fallible_clone().unwrap();
        let other_list_clone = list.fallible_clone().unwrap();

        list.append(&mut list_clone);

        assert!(list_clone.head.is_none());
        assert_eq!(list, other_list_clone);
    }

    #[test]
    fn debug() {
        let list = SinglyLinkedList::from_iter(Global, 1..4).unwrap();
        assert_eq!(std::format!("{list:?}"), String::from("[1, 2, 3]"));
    }

    #[test]
    fn drop() {
        let mut list = SinglyLinkedList::new(Global);
        for i in 0..10 {
            list.push_front(std::format!("{i}")).unwrap();
        }
    }

    #[test]
    fn front_back() {
        let mut list = SinglyLinkedList::new(Global);
        list.push_front(String::from("test1")).unwrap();
        list.push_back(String::from("test2")).unwrap();
        list.push_front(String::from("test3")).unwrap();

        assert_eq!(list.front(), Some(&String::from("test3")));
        assert_eq!(list.back(), Some(&String::from("test2")));
        assert_eq!(list.front_mut(), Some(&mut String::from("test3")));
        assert_eq!(list.back_mut(), Some(&mut String::from("test2")));
    }

    #[test]
    fn insert_get() {
        let mut list = SinglyLinkedList::new(Global);
        list.insert(0, String::from("test1")).unwrap();
        list.push_back(String::from("test2")).unwrap();
        list.push_front(String::from("test3")).unwrap();

        list.insert(1, String::from("inserted")).unwrap();
        list.insert(4, String::from("inserted2")).unwrap();
        assert_eq!(list.get(1), Some(&String::from("inserted")));

        assert_eq!(list.pop_front(), Some(String::from("test3")));
        assert_eq!(list.pop_front(), Some(String::from("inserted")));
        assert_eq!(list.pop_front(), Some(String::from("test1")));
        assert_eq!(list.pop_front(), Some(String::from("test2")));
        assert_eq!(list.pop_front(), Some(String::from("inserted2")));

        assert!(list.get(9).is_none());
        assert!(list.insert(9, String::from("foo")).is_err());
    }

    #[test]
    fn drain() {
        let mut list = SinglyLinkedList::new(Global);
        list.push_back(String::from("test1")).unwrap();
        list.push_back(String::from("test2")).unwrap();
        list.push_back(String::from("test3")).unwrap();

        assert_eq!(list.len(), 3);

        assert!(list.drain(3..).is_err());
        assert!(list.drain(0..4).is_err());
        assert!(list.drain(..=4).is_err());
        assert!(list.drain(5..6).is_err());

        let mut drained = list.drain(0..3).unwrap();
        assert_eq!(drained.next(), Some(String::from("test1")));
        assert_eq!(drained.next(), Some(String::from("test2")));
        assert_eq!(drained.next(), Some(String::from("test3")));
        assert_eq!(drained.next(), None);

        core::mem::drop(drained);

        assert!(list.head.is_none());

        let mut list = SinglyLinkedList::new(Global);
        list.push_back(String::from("test1")).unwrap();
        list.push_back(String::from("test2")).unwrap();
        list.push_back(String::from("test3")).unwrap();

        assert_eq!(list.len(), 3);

        let mut drained = list.drain(2..3).unwrap();
        assert_eq!(drained.next(), Some(String::from("test3")));
        assert_eq!(drained.next(), None);

        core::mem::drop(drained);

        assert!(list.head.is_some());
        assert_eq!(list.pop_front().unwrap(), "test1");
        assert_eq!(list.pop_front().unwrap(), "test2");
        assert!(list.pop_front().is_none());
    }

    #[test]
    fn cursor() {
        let mut list = SinglyLinkedList::new(Global);
        list.push_back(String::from("test1")).unwrap();
        list.push_back(String::from("test2")).unwrap();
        list.push_back(String::from("test3")).unwrap();

        let mut cursor = list.cursor();
        assert_eq!(cursor.current().unwrap(), "test1");
        assert_eq!(cursor.next().unwrap(), "test2");
        cursor.advance();
        assert_eq!(cursor.current().unwrap(), "test2");
        assert_eq!(cursor.next().unwrap(), "test3");
        cursor.advance();
        assert_eq!(cursor.current().unwrap(), "test3");
        assert_eq!(cursor.next(), None);
        cursor.advance();
        assert_eq!(cursor.current(), None);
    }

    #[test]
    fn cursor_mut() {
        let mut ll = SinglyLinkedList::new(Global);
        let mut cursor = ll.cursor_mut();
        cursor.insert(String::from("first")).unwrap();
        cursor.insert(String::from("second")).unwrap();
        assert_eq!(cursor.current().unwrap(), "second");
        assert_eq!(cursor.next().unwrap(), "first");
        cursor.advance();
        cursor.advance();
        cursor.insert(String::from("third")).unwrap();
        assert_eq!(ll.back().unwrap(), "third");
        assert_eq!(ll.len(), 3);

        let mut ll = SinglyLinkedList::new(Global);
        let mut cursor = ll.cursor_mut();
        assert!(cursor.current().is_none());
        assert!(cursor.next().is_none());

        cursor.insert(String::from("aaa1")).unwrap();
        assert_eq!(cursor.current().unwrap(), "aaa1");

        cursor.insert(String::from("bbb2")).unwrap();
        assert_eq!(cursor.current().unwrap(), "bbb2");
        assert_eq!(cursor.next().unwrap(), "aaa1");

        cursor.advance();
        assert_eq!(cursor.current().unwrap(), "aaa1");
        assert!(cursor.next().is_none());

        cursor.insert(String::from("ccc3")).unwrap();
        assert_eq!(cursor.next().unwrap(), "aaa1");

        cursor.insert(String::from("ddd4")).unwrap();
        assert_eq!(cursor.current().unwrap(), "ddd4");
        assert_eq!(cursor.next().unwrap(), "ccc3");
    }

    #[test]
    fn split_insert_list() {
        let mut list = SinglyLinkedList::new(Global);
        let iter = (0..10).map(|i| std::format!("{i}"));
        let fh = (0..5).map(|i| std::format!("{i}"));
        let sh = (5..10).map(|i| std::format!("{i}"));

        list.extend_from(iter.clone()).unwrap();

        let mut cursor = list.cursor_mut();

        // // 0 -> 1
        // cursor.advance();
        // // 1 -> 2
        // cursor.advance();
        // // 2 -> 3
        // cursor.advance();
        // // 3 -> 4
        // cursor.advance();
        // // 4 -> 5
        // cursor.advance();

        cursor.advance_many(5);

        let new = cursor.split();

        assert_eq!(list.len(), 5);
        assert_eq!(new.len(), 5);
    }
}
