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
    Parser, Span,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Arrow,
    Keyword(Keyword),
    Identifier(String),
    LeftAngleBracket,
    LeftBrace,
    LeftBracket,
    LeftParenthesis,
    RightAngleBracket,
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
    Enum,
    Fn,
    Service,
    String,
    Struct,
    Use,
}

pub fn lexer() -> impl Parser<Error = crate::SourceError, Output = (Token, Span), Input = char> {
    let alphabetic = (|c: &char| c.is_ascii_alphabetic()) as fn(&char) -> bool;
    hinted_choice((
        (alphabetic, identifier()),
        (';', single(';').to(Token::Semicolon)),
        (':', sequence(&[':', ':']).to(Token::PathSeparator).or(single(':').to(Token::Colon))),
        (';', single(';').to(Token::Semicolon)),
        ('(', single('(').to(Token::LeftParenthesis)),
        ('{', single('{').to(Token::LeftBrace)),
        ('[', single('[').to(Token::LeftBracket)),
        ('<', single('<').to(Token::LeftAngleBracket)),
        (')', single(')').to(Token::RightParenthesis)),
        ('}', single('}').to(Token::RightBrace)),
        (']', single(']').to(Token::RightBracket)),
        ('>', single('>').to(Token::RightAngleBracket)),
        (',', single(',').to(Token::Comma)),
        ('-', single('-').then(single('>')).to(Token::Arrow)),
    ))
    .with_span()
    .padded_by(whitespace())
}

fn identifier() -> impl Parser<Error = crate::SourceError, Output = Token, Input = char> {
    string((ascii_alphabetic(), ascii_alphanumeric() /*.or(single('_'))*/)).map(|s| match &*s {
        "enum" => Token::Keyword(Keyword::Enum),
        "fn" => Token::Keyword(Keyword::Fn),
        "struct" => Token::Keyword(Keyword::Struct),
        "service" => Token::Keyword(Keyword::Service),
        "use" => Token::Keyword(Keyword::Use),
        "String" => Token::Keyword(Keyword::String),
        _ => Token::Identifier(s),
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use comb::{
        combinators::{choice, end, many0, maybe},
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
        let lexer = lexer().map(|(v, _)| v);
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

    #[test]
    fn single_char_ident() {
        let mut stream = Stream::from_str("-I;");
        let lexer = maybe(single('-'))
            .then(string::<(), _>((ascii_alphabetic(), ascii_alphanumeric())))
            .or(end().to((Some('a'), String::from("f"))));
        assert_eq!(lexer.parse(&mut stream), Ok((Some('-'), String::from("I"))));
    }
}
