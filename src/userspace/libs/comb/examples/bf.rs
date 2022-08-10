// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use comb::{
    combinators::{choice, delimited, hinted_choice, many0, single},
    stream::{CharStream, ParserOutputStream},
    utils::{cheap_clone, TryAdapter},
    Parser,
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
    let lexer = make_lexer();
    let stream = CharStream::new("+[+-+],.");
    let mut lexer_output = ParserOutputStream::new(cheap_clone(lexer), stream);

    let parser = TryAdapter::new(many0(make_parser()));
    println!("{:?}", parser.parse(&mut lexer_output));
}

fn make_parser() -> impl Parser<Error = String, Output = Ast, Input = Token> {
    let base = cheap_clone(choice((
        single(Token::Dec).to(Ast::Dec),
        single(Token::Inc).to(Ast::Inc),
        single(Token::Input).to(Ast::Input),
        single(Token::Output).to(Ast::Output),
    )));
    choice((
        base.clone(),
        delimited(single(Token::LoopStart), many0(base), single(Token::LoopEnd)).map(|ast| Ast::Loop(ast)),
    ))
}

fn make_lexer() -> impl Parser<Error = String, Output = Token, Input = char> {
    hinted_choice((
        ('+', single('+').to(Token::Inc)),
        ('-', single('-').to(Token::Dec)),
        ('[', single('[').to(Token::LoopStart)),
        (']', single(']').to(Token::LoopEnd)),
        ('.', single('.').to(Token::Output)),
        (',', single(',').to(Token::Input)),
    ))
}
