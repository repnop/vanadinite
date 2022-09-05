// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{Choice, Hint, HintedChoice};
use crate::{error::Error, stream::Stream, Parser};

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
            I: PartialEq + core::fmt::Debug + Clone,
            E: Error,
            $($X: Parser<Error = E, Output = O, Input = I>),*
        {
            type Error = E;
            type Output = O;
            type Input = I;

            #[inline]
            fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
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

macro_rules! impl_hinted_choice {
    () => {};
    ($head:ident $hint:ident $($X:ident)*) => {
        impl_hinted_choice!($($X)*);
        impl_hinted_choice!(~ $head $hint $($X)*);
    };
    (~ $($X:ident $HX:ident)*) => {
        #[allow(unused_variables, non_snake_case)]
        impl<I, O, E, $($X, $HX,)*> Parser for HintedChoice<I, O, E, ($(($HX, $X),)*)>
        where
            I: PartialEq + core::fmt::Debug + Clone,
            E: Error,
            $($X: Parser<Error = E, Output = O, Input = I>),*,
            $(I: Hint<$HX>),*,
        {
            type Error = E;
            type Output = O;
            type Input = I;

            #[inline]
            fn parse(&self, stream: &mut Stream<'_, Self::Input>) -> Result<Self::Output, Self::Error> {
                let HintedChoice { subparsers: ($(($HX, $X),)*), .. } = self;
                let (peek, span) = stream.peek().ok_or_else(|| E::unexpected_end_of_input())?;
                let peek = peek.clone();

                $(
                    if peek.is_hinted($HX) {
                        return $X.parse(stream);
                    }
                )*

                let (val, span) = stream.next().unwrap();
                Err(E::unexpected_value(val, Some(span)))
            }
        }
    };
}

impl_hinted_choice!(A_ HA_ B_ HB_ C_ HC_ D_ HD_ E_ HE_ F_ HF_ G_ HG_ H_ HH_ I_ HI_ J_ HJ_ K_ HK_ L_ HL_ M_ HM_ N_ HN_ O_ HO_ P_ HP_ Q_ HQ_ S_ HS_ T_ HT_ U_ HU_ V_ HV_ W_ HW_ X_ HX_ Y_ HY_ Z_ HZ_);
