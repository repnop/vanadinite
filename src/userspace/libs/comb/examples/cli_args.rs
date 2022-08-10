// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use comb::{combinators::one_of, stream::CharStream, Parser};

fn main() {
    let parser = make_parser();
    parser.parse(&mut CharStream::new("]")).unwrap();
}

fn make_parser(
) -> impl Parser<Error = String, Input<'static> = char, Output = char, Stream<'static> = CharStream<'static>> {
    let chars: &'static [char] = &['[', ']', '+', '-', '.', ','];
    one_of(chars)
}
