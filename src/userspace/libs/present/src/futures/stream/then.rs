// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::{future::Future, pin::Pin};
use std::task::{Context, Poll};

use super::Stream;

pub struct Then<S, F, Fut> {
    pub(crate) stream: S,
    pub(crate) f: F,
    pub(crate) current_future: Option<Fut>,
}

impl<S, F, U, Fut> Stream for Then<S, F, Fut>
where
    S: Stream,
    F: FnMut(S::Item) -> Fut,
    Fut: Future<Output = U>,
{
    type Item = U;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        if self.current_future.is_some() {
            let current_future = unsafe { &mut self.get_unchecked_mut().current_future };
            let future = unsafe { Pin::new_unchecked(current_future.as_mut().unwrap()) };

            match future.poll(context) {
                Poll::Ready(ready) => {
                    current_future.take();
                    Poll::Ready(Some(ready))
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            let (stream, f, current_future) = unsafe {
                let this = self.get_unchecked_mut();
                (Pin::new_unchecked(&mut this.stream), &mut this.f, &mut this.current_future)
            };

            match stream.poll_next(context) {
                Poll::Ready(Some(t)) => {
                    let future = f(t);
                    let fut_ref = current_future.insert(future);
                    let pinned = unsafe { Pin::new_unchecked(fut_ref) };

                    match pinned.poll(context) {
                        Poll::Ready(ready) => {
                            current_future.take();
                            Poll::Ready(Some(ready))
                        }
                        Poll::Pending => Poll::Pending,
                    }
                }
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            }
        }
    }
}
