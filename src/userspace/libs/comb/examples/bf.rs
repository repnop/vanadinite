// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use comb::{
    combinators::{choice, delimited, end, hinted_choice, many0, many1, single},
    recursive::recursive,
    stream::{CharStream, Stream},
    text::whitespace,
    utils::cheap_clone,
    Parser, Span,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Inc,
    Dec,
    LoopStart,
    LoopEnd,
    Output,
    Input,
}

#[derive(Debug, Clone)]
enum Ast {
    Loop(Vec<Ast>),
    Inc,
    Dec,
    Output,
    Input,
}

fn main() {
    let lexer = many1(make_lexer()).then_assert(end());
    let stream = CharStream::new("+ [ + - + ] , .");
    let tokens = lexer.parse(&mut Stream::new(stream)).unwrap().0.into_iter();
    println!("{:?}", make_parser().parse(&mut Stream::new(tokens)));
}

fn make_parser() -> impl Parser<Error = String, Output = Vec<(Ast, Span)>, Input = Token> {
    many1(recursive(|this| {
        choice((
            single(Token::Dec).to(Ast::Dec),
            single(Token::Inc).to(Ast::Inc),
            single(Token::Input).to(Ast::Input),
            single(Token::Output).to(Ast::Output),
            delimited(single(Token::LoopStart), many0(this), single(Token::LoopEnd))
                .map(|ast| Ast::Loop(ast.into_iter().map(|(v, _)| v).collect())),
        ))
    }))
    .then_assert(end())
}

fn make_lexer() -> impl Parser<Error = String, Output = Token, Input = char> {
    choice((
        single('+').to(Token::Inc),
        single('-').to(Token::Dec),
        single('[').to(Token::LoopStart),
        single(']').to(Token::LoopEnd),
        single('.').to(Token::Output),
        single(',').to(Token::Input),
    ))
    .padded_by(whitespace())
}
