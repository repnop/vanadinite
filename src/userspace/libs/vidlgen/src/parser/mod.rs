// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod lexer;

use self::lexer::{Keyword, Token};
use alloc::{boxed::Box, string::String, vec::Vec};
use comb::{
    combinators::{any, consume, delimited, hinted_choice, many0, many1, maybe, single, single_by},
    recursive::recursive,
    utils::todo,
    Parser,
};

#[derive(Debug, PartialEq)]
pub enum AstNode {
    Service { name: String, methods: Vec<Method> },
}

#[derive(Debug, PartialEq)]
pub struct Method {
    pub name: String,
    pub arguments: Vec<(String, Type)>,
    pub return_type: Option<Type>,
}

#[derive(Debug, PartialEq)]
pub enum Type {
    Path(Vec<String>),
    Slice(Box<Type>),
}

pub fn parser() -> impl Parser<Error = String, Output = AstNode, Input = Token> {
    hinted_choice((
        (Token::Keyword(Keyword::Service), parse_service()),
        // ...
    ))
}

fn parse_service() -> impl Parser<Error = String, Output = AstNode, Input = Token> {
    single(Token::Keyword(Keyword::Service))
        .then_to(single_by(|t| matches!(t, Token::Identifier(_))).map(Token::into_identifier))
        .then(delimited(single(Token::LeftBrace), many1(parse_method()), single(Token::RightBrace)))
        .map(|(name, methods)| AstNode::Service { name, methods })
}

fn parse_method() -> impl Parser<Error = String, Output = Method, Input = Token> {
    single(Token::Keyword(Keyword::Fn))
        .then_to(single_by(|t| matches!(t, Token::Identifier(_))).map(Token::into_identifier))
        // .then_assert(any().map(|v| std::dbg!(v)))
        .then(delimited(single(Token::LeftParenthesis), many0(parse_argument()), single(Token::RightParenthesis)))
        .then(maybe(consume(single(Token::Arrow)).then_to(parse_type())))
        .then_assert(single(Token::Semicolon))
        .map(|((name, arguments), return_type)| Method { name, arguments, return_type })
}

fn parse_argument() -> impl Parser<Error = String, Output = (String, Type), Input = Token> {
    single_by(|t| matches!(t, Token::Identifier(_)))
        .map(Token::into_identifier)
        .then_assert(single(Token::Colon))
        .then(parse_type())
        .then_assert(single(Token::Comma))
}

fn parse_type() -> impl Parser<Error = String, Output = Type, Input = Token> {
    let identifier = (|t: &Token| matches!(t, Token::Identifier(_))) as fn(&Token) -> bool;
    recursive(move |this| {
        hinted_choice((
            (
                Token::LeftBracket,
                delimited(single(Token::LeftBracket), this, single(Token::RightBracket))
                    .map(|ty| Type::Slice(Box::new(ty))),
            ),
            (identifier, single_by(identifier).map(Token::into_identifier).map(|s| Type::Path(alloc::vec![s]))),
        ))
    })
}

#[cfg(test)]
mod test {
    use super::lexer::lexer;
    use super::*;
    use comb::stream::Stream;

    #[test]
    fn stuff_works_idk() {
        let syntax = "service MyService {
                fn fump(baz: U32, aaa: U64,) -> T2;
                fn fraz(baz2: Yeet, aaa2: [[[U64]]],) -> T;
            }";

        let tokens = many0(lexer()).parse(&mut Stream::from_str(syntax)).unwrap();
        let parser = parser();
        let mut stream = Stream::new(tokens.into_iter().map(|v| (v, comb::Span::default())));
        let mut parse = move || parser.parse(&mut stream).unwrap();

        assert_eq!(
            parse(),
            AstNode::Service {
                name: String::from("MyService"),
                methods: alloc::vec![
                    Method {
                        name: String::from("fump"),
                        arguments: alloc::vec![
                            (String::from("baz"), Type::Path(alloc::vec![String::from("U32")])),
                            (String::from("baz"), Type::Slice(Box::new(Type::Path(alloc::vec![String::from("U32")]))))
                        ],
                        return_type: Some(Type::Path(alloc::vec![String::from("T")])),
                    },
                    Method {
                        name: String::from("fraz"),
                        arguments: alloc::vec![
                            (String::from("baz"), Type::Path(alloc::vec![String::from("Yeet")])),
                            (
                                String::from("baz"),
                                Type::Slice(Box::new(Type::Slice(Box::new(Type::Slice(Box::new(Type::Path(
                                    alloc::vec![String::from("U32")]
                                )))))))
                            )
                        ],
                        return_type: None,
                    },
                ]
            }
        );
    }
}
