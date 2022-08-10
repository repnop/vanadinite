// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{error::Error, stream::Stream, Parser};

pub struct Choice<I, O, E: Error, P> {
    pub(super) subparsers: P,
    pub(super) _i: core::marker::PhantomData<I>,
    pub(super) _o: core::marker::PhantomData<O>,
    pub(super) _e: core::marker::PhantomData<E>,
}

macro_rules! impl_choice {
    () => {};
    ($head:ident $($X:ident)*) => {
        impl_choice!($($X)*);
        impl_choice!(~ $head $($X)*);
    };
    (~ $($X:ident)*) => {
        #[allow(unused_variables, non_snake_case)]
        impl<I, O, E, $($X,)*> Parser for Choice<I, O, E, ($($X,)*)>
        where
            I: PartialEq + core::fmt::Debug,
            E: Error,
            $($X: Parser<Error = E, Output = O, Input = I>),*
        {
            type Error = E;
            type Output = O;
            type Input = I;

            #[inline]
            fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
            where
                S: Stream<Item = Self::Input>,
            {
                let Choice { subparsers: ($($X,)*), .. } = self;

                $(
                    if let Ok(output) = $X.try_parse(stream) {
                        return Ok(output);
                    }
                )*

                let (val, span) = stream.next().ok_or_else(|| E::unexpected_end_of_input())?;
                Err(E::unexpected_value(val, Some(span)))
            }
        }
    };
}

impl_choice!(A_ B_ C_ D_ E_ F_ G_ H_ I_ J_ K_ L_ M_ N_ O_ P_ Q_ S_ T_ U_ V_ W_ X_ Y_ Z_);

pub struct HintedChoice<I, O, E: Error, P> {
    pub(super) subparsers: P,
    pub(super) _i: core::marker::PhantomData<I>,
    pub(super) _o: core::marker::PhantomData<O>,
    pub(super) _e: core::marker::PhantomData<E>,
}

macro_rules! impl_hinted_choice {
    () => {};
    ($head:ident $($X:ident)*) => {
        impl_hinted_choice!($($X)*);
        impl_hinted_choice!(~ $head $($X)*);
    };
    (~ $($X:ident)*) => {
        #[allow(unused_variables, non_snake_case)]
        impl<I, O, E, $($X,)*> Parser for HintedChoice<I, O, E, ($((I, $X),)*)>
        where
            I: PartialEq + core::fmt::Debug + Clone,
            E: Error,
            $($X: Parser<Error = E, Output = O, Input = I>),*
        {
            type Error = E;
            type Output = O;
            type Input = I;

            #[inline]
            fn parse<S>(&self, stream: &mut S) -> Result<Self::Output, Self::Error>
            where
                S: Stream<Item = Self::Input>,
            {
                let HintedChoice { subparsers: ($($X,)*), .. } = self;
                let (peek, span) = stream.peek().ok_or_else(|| E::unexpected_end_of_input())?;
                let peek = peek.clone();

                $(
                    if peek == $X.0 {
                        if let Ok(output) = $X.1.try_parse(stream) {
                            return Ok(output);
                        }
                    }
                )*

                let (val, span) = stream.next().unwrap();
                Err(E::unexpected_value(val, Some(span)))
            }
        }
    };
}

impl_hinted_choice!(A_ B_ C_ D_ E_ F_ G_ H_ I_ J_ K_ L_ M_ N_ O_ P_ Q_ S_ T_ U_ V_ W_ X_ Y_ Z_);
