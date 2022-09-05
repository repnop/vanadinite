// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use alloc::string::String;
use comb::{
    combinators::{hinted_choice, sequence, single},
    text::{ascii_alphabetic, ascii_alphanumeric, string, whitespace},
    Parser,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Arrow,
    Keyword(Keyword),
    Identifier(String),
    LeftBrace,
    LeftBracket,
    LeftParenthesis,
    RightBrace,
    RightBracket,
    RightParenthesis,
    PathSeparator,
    Colon,
    Semicolon,
    Comma,
}

impl Token {
    pub fn into_identifier(self) -> String {
        match self {
            Self::Identifier(s) => s,
            _ => panic!("attempted to unwrap an identifier"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Fn,
    Service,
}

pub fn lexer() -> impl Parser<Error = String, Output = Token, Input = char> {
    let alphabetic = (|c: &char| c.is_ascii_alphabetic()) as fn(&char) -> bool;
    hinted_choice((
        (alphabetic, identifier()),
        (';', single(';').to(Token::Semicolon)),
        (':', sequence(&[':', ':']).to(Token::PathSeparator).or(single(':').to(Token::Colon))),
        (';', single(';').to(Token::Semicolon)),
        ('(', single('(').to(Token::LeftParenthesis)),
        ('{', single('{').to(Token::LeftBrace)),
        ('[', single('[').to(Token::LeftBracket)),
        (')', single(')').to(Token::RightParenthesis)),
        ('}', single('}').to(Token::RightBrace)),
        (']', single(']').to(Token::RightBracket)),
        (',', single(',').to(Token::Comma)),
        ('-', single('-').then(single('>')).to(Token::Arrow)),
    ))
    .padded_by(whitespace())
}

fn identifier() -> impl Parser<Error = String, Output = Token, Input = char> {
    string((ascii_alphabetic(), ascii_alphanumeric() /*.or(single('_'))*/)).map(|s| match &*s {
        "fn" => Token::Keyword(Keyword::Fn),
        "service" => Token::Keyword(Keyword::Service),
        _ => Token::Identifier(s),
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use comb::{
        combinators::{choice, end, many0},
        stream::{CharStream, Stream},
        Span,
    };

    #[test]
    fn some_stuff() {
        let some_syntax = r#"ThisIsAnIdent I
:
::
()
[]
{}
->
fn
service
        "#;

        let mut stream = Stream::new(CharStream::new(some_syntax));
        let lexer = lexer();
        let mut lexer_parse = move || lexer.parse(&mut stream);

        assert_eq!(lexer_parse(), Ok(Token::Identifier(String::from("ThisIsAnIdent"))));
        assert_eq!(lexer_parse(), Ok(Token::Identifier(String::from("I"))));
        assert_eq!(lexer_parse(), Ok(Token::Colon));
        assert_eq!(lexer_parse(), Ok(Token::PathSeparator));
        assert_eq!(lexer_parse(), Ok(Token::LeftParenthesis));
        assert_eq!(lexer_parse(), Ok(Token::RightParenthesis));
        assert_eq!(lexer_parse(), Ok(Token::LeftBracket));
        assert_eq!(lexer_parse(), Ok(Token::RightBracket));
        assert_eq!(lexer_parse(), Ok(Token::LeftBrace));
        assert_eq!(lexer_parse(), Ok(Token::RightBrace));
        assert_eq!(lexer_parse(), Ok(Token::Arrow));
        assert_eq!(lexer_parse(), Ok(Token::Keyword(Keyword::Fn)));
        assert_eq!(lexer_parse(), Ok(Token::Keyword(Keyword::Service)));
    }
}
