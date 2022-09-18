// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
};
use comb::Parser;
use parser::{AstNode, Method, Service, Type, Use};

extern crate alloc;
#[cfg(test)]
extern crate std;

mod parser;

#[derive(Default)]
pub struct Compiler {
    providers: BTreeMap<String, String>,
    usages: BTreeMap<String, String>,
}

impl Compiler {
    pub fn new() -> Self {
        let mut this = Self { providers: BTreeMap::new(), usages: BTreeMap::new() };
        this.usages.insert(String::from("U8"), String::from("core::U8"));
        this.usages.insert(String::from("String"), String::from("core::String"));
        this.usages.insert(String::from("Result"), String::from("core::Result"));
        this.provider("core", "vidl::core")
    }

    pub fn provider(mut self, namespace: &str, dep: &str) -> Self {
        self.providers.insert(namespace.to_string(), dep.to_string());
        self
    }

    pub fn compile(&mut self, source: &str) -> Result<CompiledVidl, CompileError> {
        let stream = comb::combinators::many1(parser::lexer::lexer())
            .then_assert(comb::combinators::end())
            .parse(&mut comb::stream::Stream::from_str(source))?;
        let mut stream = comb::stream::Stream::new(stream.into_iter());
        let mut ast = alloc::vec::Vec::new();

        loop {
            let res = parser::parser().parse(&mut stream);

            match (res, stream.peek()) {
                (Err(_), None) => break,
                (Err(e), Some(_)) => return Err(e.into()),
                (Ok(node), _) => match node {
                    AstNode::Use(_use) => match _use {
                        Use::FullPath(idents) => {
                            drop(self.usages.insert(idents.last().cloned().unwrap(), idents.join("::")))
                        }
                        Use::Grouped { .. } => {
                            for use_path in _use.flatten() {
                                self.usages.insert(use_path.last().cloned().unwrap(), use_path.join("::"));
                            }
                        }
                    },
                    node => ast.push(node),
                },
            }
        }

        let mut compiled = CompiledVidl { output: String::new() };

        for node in ast {
            match node {
                AstNode::Service(service) => self.lower_service(&mut compiled, &service)?,
                AstNode::Use(_) => unreachable!(),
            }
        }

        Ok(compiled)
    }

    fn lower_service(&self, compiled: &mut CompiledVidl, service: &Service) -> Result<(), CompileError> {
        self.lower_service_server(compiled, service)?;
        self.lower_service_client(compiled, service)
    }

    fn lower_service_server(&self, compiled: &mut CompiledVidl, service: &Service) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!("trait {} {{", service.name));
        for method in &service.methods {
            self.lower_method_server(compiled, method)?;
        }
        compiled.write_str("\n}\n\n");

        Ok(())
    }

    fn lower_service_client(&self, compiled: &mut CompiledVidl, service: &Service) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!("trait {}Client {{", service.name));
        for method in &service.methods {
            self.lower_method_client(compiled, method)?;
        }
        compiled.write_str("\n}\n\n");

        Ok(())
    }

    fn lower_method_server(&self, compiled: &mut CompiledVidl, method: &Method) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!("\n    fn {}(", method.name));

        for (i, arg) in method.arguments.iter().enumerate() {
            compiled.write_fmt(format_args!("{}: ", arg.0));
            self.lower_type(compiled, &arg.1, false)?;
            if i + 1 != method.arguments.len() {
                compiled.write_str(",");
            }
        }

        compiled.write_str(")");

        if let Some(ret_type) = &method.return_type {
            compiled.write_str(" -> ");
            self.lower_type(compiled, ret_type, true)?;
        }

        compiled.write_str(";");

        Ok(())
    }

    fn lower_method_client(&self, compiled: &mut CompiledVidl, method: &Method) -> Result<(), CompileError> {
        // compiled.write_fmt(format_args!("fn "));

        Ok(())
    }

    fn lower_type(&self, compiled: &mut CompiledVidl, ty: &Type, in_return_position: bool) -> Result<(), CompileError> {
        match ty {
            Type::Path { path, generics } => {
                // TODO: check for import vs defined in scope here
                let path = match self.usages.get(path.first().unwrap()) {
                    None => path.join("::"),
                    Some(full_path) => {
                        let name = full_path.split("::").next().unwrap();
                        let mut out_path = self.providers.get(name).unwrap().clone();

                        for rest in full_path.split("::").skip(1).chain(path.iter().map(String::as_str).skip(1)) {
                            out_path.push_str("::");
                            out_path.push_str(rest);
                        }

                        out_path
                    }
                };
                let generics = match generics {
                    Some(generics) => {
                        let mut tmp_compiled = CompiledVidl { output: String::from("<") };
                        generics.iter().enumerate().try_for_each(|(i, ty)| {
                            let ret = self.lower_type(&mut tmp_compiled, ty, in_return_position);
                            if i + 1 != generics.len() {
                                tmp_compiled.write_str(", ");
                            }
                            ret
                        })?;
                        tmp_compiled.output.push('>');

                        tmp_compiled.output
                    }
                    None => String::new(),
                };
                compiled.write_fmt(format_args!("{}{}", path, generics));
            }
            Type::Slice(inner_ty) => match in_return_position {
                true => {
                    compiled.write_str("vidl::core::Vec<");
                    self.lower_type(compiled, inner_ty, in_return_position)?;
                    compiled.write_str(">");
                }
                false => {
                    compiled.write_str("&[");
                    self.lower_type(compiled, inner_ty, in_return_position)?;
                    compiled.write_str("]");
                }
            },
        }

        Ok(())
    }
}

pub struct CompiledVidl {
    output: String,
}

impl CompiledVidl {
    fn write_str(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn write_fmt(&mut self, f: core::fmt::Arguments<'_>) {
        use core::fmt::Write;
        let _ = self.output.write_fmt(f);
    }
}

impl core::fmt::Display for CompiledVidl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.output.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub enum CompileError {
    SourceError(SourceError),
}

impl CompileError {
    pub fn display_with<'a>(&'a self, source: &'a str) -> ErrorPrettyDisplay<'a> {
        ErrorPrettyDisplay { error: self, source }
    }
}

impl From<SourceError> for CompileError {
    fn from(e: SourceError) -> Self {
        Self::SourceError(e)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct SourceError {
    pub kind: SourceErrorKind,
    pub span: Option<comb::Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceErrorKind {
    Custom(String),
    UnexpectedCharacter(char),
    UnexpectedEnd,
}

impl comb::error::Error for SourceError {
    fn custom<E: core::fmt::Display>(error: E, span: Option<comb::Span>) -> Self {
        Self { kind: SourceErrorKind::Custom(error.to_string()), span }
    }

    fn expected_one_of<V: core::fmt::Debug, S: AsRef<[V]>>(found: V, values: S, span: Option<comb::Span>) -> Self {
        use core::fmt::Write;

        let mut s = alloc::string::String::from("expected one of ");

        for (i, v) in values.as_ref().iter().enumerate() {
            match i {
                0 => write!(&mut s, "`{:?}`", v).unwrap(),
                _ => write!(&mut s, ", `{:?}`", v).unwrap(),
            }
        }

        match span {
            Some(span) => write!(&mut s, " @ {}, found `{:?}`", span, found).unwrap(),
            None => write!(&mut s, ", found `{:?}`", found).unwrap(),
        }

        Self::custom(s, span)
    }

    fn unexpected_end_of_input() -> Self {
        Self { kind: SourceErrorKind::UnexpectedEnd, span: None }
    }

    fn unexpected_value<V: core::fmt::Debug>(value: V, span: Option<comb::Span>) -> Self {
        Self::custom(alloc::format!("unexpected value `{:?}`", value), span)
    }
}

pub struct ErrorPrettyDisplay<'a> {
    error: &'a CompileError,
    source: &'a str,
}

impl ErrorPrettyDisplay<'_> {
    fn line_containing_error(&self, span: comb::Span) -> (usize, &str, usize) {
        let mut pos = 0;
        self.source
            .split('\n')
            .enumerate()
            .find(|(_, line)| {
                let new_pos = pos + line.len() + 1;
                if span.start <= new_pos && span.end <= new_pos {
                    true
                } else {
                    pos = new_pos;
                    false
                }
            })
            .map(|(ln, l)| (pos, l, ln))
            .unwrap()
    }

    fn display_inner(
        &self,
        span: Option<comb::Span>,
        f: &mut core::fmt::Formatter<'_>,
        msg: &dyn core::fmt::Display,
    ) -> core::fmt::Result {
        match span {
            None => write!(f, "{}", msg),
            Some(span) => {
                let (char_start, line, line_number) = self.line_containing_error(span);
                let offset = span.start - char_start;
                writeln!(f, "{:>3} | {}", line_number, line)?;
                writeln!(f, "    | {:>width$}{} {}", " ", "^".repeat(span.end - span.start), msg, width = offset)
            }
        }
    }
}

impl core::fmt::Display for ErrorPrettyDisplay<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.error {
            CompileError::SourceError(source_error) => match &source_error.kind {
                SourceErrorKind::Custom(custom) => self.display_inner(source_error.span, f, custom),
                SourceErrorKind::UnexpectedCharacter(c) => self.display_inner(source_error.span, f, c),
                SourceErrorKind::UnexpectedEnd => self.display_inner(None, f, &"unexpected end of input"),
            },
        }
    }
}
