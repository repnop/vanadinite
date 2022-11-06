// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::pin::Pin;
use std::task::{Context, Poll};

use super::{helpers::AlternatingPollOrder, Stream};

#[derive(Debug)]
#[must_use = "`Future`s must be awaited or polled to do anything"]
pub struct Merge<S1: Stream, S2: Stream<Item = S1::Item>> {
    pub(super) helper: AlternatingPollOrder<S1, S2>,
}

impl<S1: Stream, S2: Stream<Item = S1::Item>> Stream for Merge<S1, S2> {
    type Item = S1::Item;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        unsafe { self.map_unchecked_mut(|s| &mut s.helper) }.poll_next(context)
    }
}
