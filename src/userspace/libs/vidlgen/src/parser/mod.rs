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
    combinators::{delimited, hinted_choice, many1, maybe, single, single_by, until},
    recursive::recursive,
    Parser,
};

#[derive(Debug, PartialEq)]
pub enum AstNode {
    Service(Service),
    Use(Use),
    TypeDefinition(Vec<String>, TypeDefinition),
}

#[derive(Debug, PartialEq)]
pub enum TypeDefinition {
    Struct(Struct),
    Enum(Enum),
}

#[derive(Debug, PartialEq)]
pub enum Use {
    FullPath(Vec<String>),
    Grouped { base: Vec<String>, branches: Vec<Self> },
}

impl Use {
    pub fn flatten(self) -> Vec<Vec<String>> {
        match self {
            Use::FullPath(path) => alloc::vec![path],
            Use::Grouped { base, branches } => branches
                .into_iter()
                .flat_map(|u| {
                    Self::flatten(u).into_iter().map(|i| {
                        let mut concat = base.clone();
                        concat.extend(i);
                        concat
                    })
                })
                .collect(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Service {
    pub name: String,
    pub methods: Vec<Method>,
}

#[derive(Debug, PartialEq)]
pub struct Method {
    pub name: String,
    pub arguments: Vec<(String, Type)>,
    pub return_type: Option<Type>,
}

#[derive(Debug, PartialEq)]
pub struct Struct {
    pub name: String,
    pub generics: Option<Vec<String>>,
    pub fields: Vec<Field>,
}

#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, PartialEq)]
pub struct Enum {
    pub name: String,
    pub generics: Option<Vec<String>>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, PartialEq)]
pub struct Variant {
    pub name: String,
    pub associated_data: Option<VariantData>,
}

#[derive(Debug, PartialEq)]
pub enum VariantData {
    Tuple(Vec<Type>),
    Struct(Vec<Field>),
}

#[derive(Debug, PartialEq)]
pub enum Type {
    Array(Box<Type>, usize),
    Path { path: Vec<String>, generics: Option<Vec<Type>> },
    Slice(Box<Type>),
    Str,
}

pub fn parser() -> impl Parser<Error = crate::SourceError, Output = AstNode, Input = Token> {
    hinted_choice((
        (Token::Keyword(Keyword::Service), parse_service()),
        (Token::Keyword(Keyword::Use), parse_use()),
        (
            Token::Keyword(Keyword::Struct),
            parse_struct_definition().map(|d| AstNode::TypeDefinition(alloc::vec![], TypeDefinition::Struct(d))),
        ),
        (
            Token::Keyword(Keyword::Enum),
            parse_enum_definition().map(|d| AstNode::TypeDefinition(alloc::vec![], TypeDefinition::Enum(d))),
        ),
        (Token::At, parse_attributes_then_type_definition()),
        // ...
    ))
}

fn parse_use() -> impl Parser<Error = crate::SourceError, Output = AstNode, Input = Token> {
    single(Token::Keyword(Keyword::Use))
        .then_to(parse_use_list())
        .then_assert(single(Token::Semicolon))
        .map(AstNode::Use)
}

fn parse_use_list() -> impl Parser<Error = crate::SourceError, Output = Use, Input = Token> {
    parse_use_full_path().or(parse_use_grouped())
}

fn parse_use_full_path() -> impl Parser<Error = crate::SourceError, Output = Use, Input = Token> {
    parse_ident().separated_by(single(Token::PathSeparator)).map(Use::FullPath)
}

fn parse_use_grouped() -> impl Parser<Error = crate::SourceError, Output = Use, Input = Token> {
    recursive(|this| {
        until(Token::LeftBrace, parse_ident().then_assert(single(Token::PathSeparator)))
            .then(delimited(
                single(Token::LeftBrace),
                parse_use_full_path().or(this).separated_by(single(Token::Comma)).allow_trailing(),
                single(Token::RightBrace),
            ))
            .map(|(base, branches)| Use::Grouped { base, branches })
    })
}

fn parse_service() -> impl Parser<Error = crate::SourceError, Output = AstNode, Input = Token> {
    single(Token::Keyword(Keyword::Service))
        .then_to(parse_ident())
        .then(delimited(single(Token::LeftBrace), until(Token::RightBrace, parse_method()), single(Token::RightBrace)))
        .map(|(name, methods)| AstNode::Service(Service { name, methods }))
}

fn parse_method() -> impl Parser<Error = crate::SourceError, Output = Method, Input = Token> {
    single(Token::Keyword(Keyword::Fn))
        .then_to(single_by(|t| matches!(t, Token::Identifier(_))).map(Token::into_identifier))
        .then(delimited(
            single(Token::LeftParenthesis),
            parse_argument().separated_by(single(Token::Comma)).allow_trailing(),
            single(Token::RightParenthesis),
        ))
        .then(maybe(single(Token::Arrow).then_to(parse_type())))
        .then_assert(single(Token::Semicolon))
        .map(|((name, arguments), return_type)| Method { name, arguments, return_type })
}

fn parse_argument() -> impl Parser<Error = crate::SourceError, Output = (String, Type), Input = Token> {
    single_by(|t| matches!(t, Token::Identifier(_)))
        .map(Token::into_identifier)
        .then_assert(single(Token::Colon))
        .then(parse_type())
}

fn parse_type() -> impl Parser<Error = crate::SourceError, Output = Type, Input = Token> {
    let identifier = (|t: &Token| matches!(t, Token::Identifier(_))) as fn(&Token) -> bool;
    let number = (|t: &Token| matches!(t, Token::Number(_))) as fn(&Token) -> bool;
    recursive(move |this| {
        hinted_choice((
            (
                Token::LeftBracket,
                delimited(
                    single(Token::LeftBracket),
                    this.clone()
                        .then(maybe(single(Token::Semicolon).then_to(single_by(number).map(Token::into_number)))),
                    single(Token::RightBracket),
                )
                .map(|(ty, maybe_array)| match maybe_array {
                    None => Type::Slice(Box::new(ty)),
                    Some(n) => Type::Array(Box::new(ty), n),
                }),
            ),
            (Token::Keyword(Keyword::String), single(Token::Keyword(Keyword::String)).map(|_| Type::Str)),
            (
                identifier,
                single_by(identifier)
                    .map(Token::into_identifier)
                    .separated_by(single(Token::PathSeparator))
                    .then(maybe(delimited(
                        single(Token::LeftAngleBracket),
                        this.separated_by(single(Token::Comma)).allow_trailing(),
                        single(Token::RightAngleBracket),
                    )))
                    .map(|(path, generics)| Type::Path { path, generics }),
            ),
        ))
    })
}

fn parse_ident() -> impl Parser<Error = crate::SourceError, Output = String, Input = Token> {
    single_by(|t| matches!(t, Token::Identifier(_))).map(Token::into_identifier)
}

fn parse_attributes_then_type_definition() -> impl Parser<Error = crate::SourceError, Output = AstNode, Input = Token> {
    many1(single(Token::At).then_to(parse_ident()))
        .then(hinted_choice((
            (Token::Keyword(Keyword::Struct), parse_struct_definition().map(TypeDefinition::Struct)),
            (Token::Keyword(Keyword::Enum), parse_enum_definition().map(TypeDefinition::Enum)),
        )))
        .map(|(attrs, def)| AstNode::TypeDefinition(attrs, def))
}

fn parse_struct_definition() -> impl Parser<Error = crate::SourceError, Output = Struct, Input = Token> {
    single(Token::Keyword(Keyword::Struct))
        .then_to(parse_ident())
        .then(maybe(delimited(
            single(Token::LeftAngleBracket),
            parse_ident().separated_by(single(Token::Comma)).allow_trailing(),
            single(Token::RightAngleBracket),
        )))
        .then(delimited(
            single(Token::LeftBrace),
            parse_ident()
                .then_assert(single(Token::Colon))
                .then(parse_type())
                .map(|(name, ty)| Field { name, ty })
                .separated_by(single(Token::Comma))
                .allow_trailing(),
            single(Token::RightBrace),
        ))
        .map(|((name, generics), fields)| Struct { name, generics, fields })
}

fn parse_enum_definition() -> impl Parser<Error = crate::SourceError, Output = Enum, Input = Token> {
    single(Token::Keyword(Keyword::Enum))
        .then_to(parse_ident())
        .then(maybe(delimited(
            single(Token::LeftAngleBracket),
            parse_ident().separated_by(single(Token::Comma)).allow_trailing(),
            single(Token::RightAngleBracket),
        )))
        .then(delimited(
            single(Token::LeftBrace),
            parse_ident()
                .then(maybe(parse_enum_variant_data()))
                .map(|(name, associated_data)| Variant { name, associated_data })
                .separated_by(single(Token::Comma))
                .allow_trailing(),
            single(Token::RightBrace),
        ))
        .map(|((name, generics), variants)| Enum { name, generics, variants })
}

fn parse_enum_variant_data() -> impl Parser<Error = crate::SourceError, Output = VariantData, Input = Token> {
    hinted_choice((
        (
            Token::LeftBrace,
            delimited(
                single(Token::LeftBrace),
                parse_ident()
                    .then_assert(single(Token::Colon))
                    .then(parse_type())
                    .map(|(name, ty)| Field { name, ty })
                    .separated_by(single(Token::Comma))
                    .allow_trailing(),
                single(Token::RightBrace),
            )
            .map(VariantData::Struct),
        ),
        (
            Token::LeftParenthesis,
            delimited(
                single(Token::LeftParenthesis),
                parse_type().separated_by(single(Token::Comma)).allow_trailing(),
                single(Token::RightParenthesis),
            )
            .map(VariantData::Tuple),
        ),
    ))
}

#[cfg(test)]
mod test {
    use super::lexer::lexer;
    use super::*;
    use comb::stream::{CharStream, Stream};

    struct DebugWrite;

    impl core::fmt::Write for DebugWrite {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            use std::io::Write;
            std::io::stdout().lock().write_all(s.as_bytes()).unwrap();
            Ok(())
        }
    }

    #[test]
    fn stuff_works_idk() {
        let syntax = "service MyService {
                fn fump(baz: U32, aaa: U64) -> T;
                fn fraz (baz2: Yeet, aaa2: [[[U64]]]) -> Foo<Baz, Bar>;
            }";

        let tokens = comb::combinators::many0(lexer()).parse(&mut Stream::from_str(syntax)).unwrap();
        let parser = parser();
        let mut stream = Stream::new(tokens.into_iter());
        let mut parse = move || parser.parse(&mut stream).unwrap();

        assert_eq!(
            parse(),
            AstNode::Service(Service {
                name: String::from("MyService"),
                methods: alloc::vec![
                    Method {
                        name: String::from("fump"),
                        arguments: alloc::vec![
                            (
                                String::from("baz"),
                                Type::Path { path: alloc::vec![String::from("U32")], generics: None }
                            ),
                            (
                                String::from("aaa"),
                                Type::Path { path: alloc::vec![String::from("U64")], generics: None }
                            ),
                        ],
                        return_type: Some(Type::Path { path: alloc::vec![String::from("T")], generics: None }),
                    },
                    Method {
                        name: String::from("fraz"),
                        arguments: alloc::vec![
                            (
                                String::from("baz2"),
                                Type::Path { path: alloc::vec![String::from("Yeet")], generics: None }
                            ),
                            (
                                String::from("aaa2"),
                                Type::Slice(Box::new(Type::Slice(Box::new(Type::Slice(Box::new(Type::Path {
                                    path: alloc::vec![String::from("U64")],
                                    generics: None,
                                }))))))
                            )
                        ],
                        return_type: Some(Type::Path {
                            path: alloc::vec![String::from("Foo")],
                            generics: Some(alloc::vec![
                                Type::Path { path: alloc::vec![String::from("Baz")], generics: None },
                                Type::Path { path: alloc::vec![String::from("Bar")], generics: None }
                            ])
                        }),
                    },
                ]
            })
        );
    }
}
