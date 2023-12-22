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
use parser::{AstNode, Enum, Method, Service, Struct, Type, TypeDefinition, Use};

extern crate alloc;
#[cfg(test)]
extern crate std;

mod parser;

#[derive(Default)]
pub struct Compiler {
    providers: BTreeMap<String, String>,
    usages: BTreeMap<String, String>,
    generate_async: bool,
}

impl Compiler {
    pub fn new(generate_async: bool) -> Self {
        let mut this = Self { providers: BTreeMap::new(), usages: BTreeMap::new(), generate_async };
        this.usages.insert(String::from("Result"), String::from("core::Result"));
        this.usages.insert(String::from("Option"), String::from("core::Option"));
        this.provider("sync", "vidl::sync").provider("core", "vidl::core")
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

        // if self.generate_async {
        //     compiled.write_str("#![feature(async_fn_in_trait)]\n#![allow(incomplete_features)]\n\n");
        // }

        for node in ast {
            match node {
                AstNode::Service(service) => self.lower_service(&mut compiled, &service)?,
                AstNode::Use(_) => unreachable!(),
                AstNode::TypeDefinition(attrs, typedef) => self.lower_typedef(&mut compiled, &attrs, &typedef)?,
            }
        }

        Ok(compiled)
    }

    fn lower_service(&self, compiled: &mut CompiledVidl, service: &Service) -> Result<(), CompileError> {
        for (i, method) in service.methods.iter().enumerate() {
            compiled.write_fmt(format_args!(
                r"#[allow(non_upper_case_globals)]
const {}_{}_ID: usize = {};
",
                service.name.to_uppercase(),
                method.name.to_uppercase(),
                i
            ));
        }
        self.lower_service_server(compiled, service)?;
        self.lower_service_client(compiled, service)
    }

    fn lower_service_server(&self, compiled: &mut CompiledVidl, service: &Service) -> Result<(), CompileError> {
        if self.generate_async {
            self.lower_service_server_async(compiled, service)?;
        }

        compiled.write_fmt(format_args!(
            "pub trait {}Provider {{
    type Error;",
            service.name
        ));
        for method in &service.methods {
            self.lower_method_server(compiled, method)?;
        }
        compiled.write_str("\n}\n\n");
        compiled.write_fmt(format_args!(r#"pub struct {0}<T: {0}Provider>(T);

impl<T: {0}Provider> {0}<T> {{
    pub fn new(provider: T) -> Self {{ Self(provider) }}
    pub fn serve(&mut self) -> ! {{
        loop {{
            let vidl::internal::KernelMessage::NewEndpointMessage(cptr) = vidl::internal::read_kernel_message() else {{ continue }};
            let channel = vidl::internal::IpcChannel::new(cptr);
            let Ok((msg, mut caps)) = channel.read_with_all_caps(vidl::internal::ChannelReadFlags::NONBLOCKING) else {{ continue }};
            if caps.is_empty() {{
                // Need at least one cap for the RPC message
                continue;
            }}

            // If its not a memory cap we got a problem
            let vidl::CapabilityWithDescription {{
                capability: _,
                description: vidl::CapabilityDescription::Memory {{ ptr, len, permissions: vidl::internal::MemoryPermissions::READ_WRITE }},
            }} = caps.remove(0) else {{ continue }};
            let buffer = unsafe {{ core::slice::from_raw_parts(ptr, len) }};

            match msg.0[0] {{
"#, service.name));

        for method in &service.methods {
            compiled.write_fmt(format_args!(
                r#"                {}_{}_ID => {{
                let deserializer = vidl::materialize::Deserializer::new(buffer, &caps[..]);
                let Ok(("#,
                service.name.to_uppercase(),
                method.name.to_uppercase()
            ));
            for arg in &method.arguments {
                compiled.write_fmt(format_args!("{},", arg.0));
            }
            compiled.write_str(")) = deserializer.deserialize::<(");
            for arg in &method.arguments {
                self.lower_type(compiled, &arg.1, true)?;
                compiled.write_str(", ");
            }
            compiled.write_str(")>() else { continue };\n");
            compiled.write_fmt(format_args!("                if let Ok(response) = self.0.{}(", method.name));
            for (i, arg) in method.arguments.iter().enumerate() {
                compiled.write_fmt(format_args!("{}", arg.0));
                if i + 1 != method.arguments.len() {
                    compiled.write_str(", ");
                }
            }
            compiled.write_fmt(format_args!(
                r#") {{
                    let mut serializer = vidl::materialize::Serializer::new();
                    serializer.serialize(&response).unwrap();
                    let (buffer, mut caps) = serializer.into_parts();
                    let mut mem = vidl::SharedMemoryAllocation::public_rw(vidl::Bytes(buffer.len())).unwrap();
                    unsafe {{ mem.as_mut()[..buffer.len()].copy_from_slice(&buffer) }};
                    caps.insert(0, vidl::Capability {{ cptr: mem.cptr, rights: vidl::CapabilityRights::READ }});
                    let _ = channel.send(vidl::EndpointMessage([{}_{}_ID, 0, 0, 0, 0, 0, 0]), &caps[..]);
                }}"#,
                service.name.to_uppercase(),
                method.name.to_uppercase()
            ));

            compiled.write_str("            },\n");
        }

        compiled.write_str(
            r#"                _ => {},
            }
        }
    }
}
        "#,
        );

        Ok(())
    }

    fn lower_service_client(&self, compiled: &mut CompiledVidl, service: &Service) -> Result<(), CompileError> {
        if self.generate_async {
            self.lower_service_client_async(compiled, service)?;
        }

        compiled.write_fmt(format_args!(
            r"pub struct {0}Client(std::ipc::IpcChannel);

impl {0}Client {{

",
            service.name
        ));

        compiled.write_str(
            r"    pub fn new(cptr: std::ipc::CapabilityPtr) -> Self { Self(std::ipc::IpcChannel::new(cptr)) }

",
        );

        for method in &service.methods {
            self.lower_method_client(compiled, method, service)?;
        }
        compiled.write_str("\n}\n\n");

        Ok(())
    }

    fn lower_method_server(&self, compiled: &mut CompiledVidl, method: &Method) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!("\n    fn {}(&mut self, ", method.name));

        for (i, arg) in method.arguments.iter().enumerate() {
            compiled.write_fmt(format_args!("{}: ", arg.0));
            self.lower_type(compiled, &arg.1, true)?;
            if i + 1 != method.arguments.len() {
                compiled.write_str(", ");
            }
        }

        compiled.write_str(")");

        match &method.return_type {
            Some(ret_type) => {
                compiled.write_str(" -> Result<");
                self.lower_type(compiled, ret_type, true)?;
                compiled.write_str(", Self::Error>")
            }
            None => compiled.write_str(" -> Result<(), Self::Error>"),
        }

        compiled.write_str(";");

        Ok(())
    }

    fn lower_method_client(
        &self,
        compiled: &mut CompiledVidl,
        method: &Method,
        service: &Service,
    ) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!("    pub fn {}(&self, ", method.name));
        for (i, arg) in method.arguments.iter().enumerate() {
            compiled.write_fmt(format_args!("{}: ", arg.0));
            self.lower_type(compiled, &arg.1, false)?;
            if i + 1 != method.arguments.len() {
                compiled.write_str(",");
            }
        }

        compiled.write_str(")");

        if let Some(ret_type) = &method.return_type {
            compiled.write_str(" ->");
            self.lower_type(compiled, ret_type, true)?;
        }

        compiled.write_str(
            r" {
        let mut serializer = vidl::materialize::Serializer::new();
        serializer.serialize(&(",
        );

        for arg in &method.arguments {
            compiled.write_fmt(format_args!("&{},", arg.0));
        }

        compiled.write_fmt(format_args!(r#")).unwrap();
        let (buffer, mut caps) = serializer.into_parts();
        let mut mem = vidl::SharedMemoryAllocation::public_rw(vidl::Bytes(buffer.len())).unwrap();
        unsafe {{ mem.as_mut()[..buffer.len()].copy_from_slice(&buffer) }};
        caps.insert(0, vidl::Capability {{ cptr: mem.cptr, rights: vidl::CapabilityRights::READ }});
        self.0.send(vidl::EndpointMessage([{}_{}_ID, 0, 0, 0, 0, 0, 0]), &caps[..]).unwrap();
        let (_msg, mut caps) = self.0.read_with_all_caps(vidl::ChannelReadFlags::NONE).unwrap();
        let _ = vidl::internal::read_kernel_message();

        match caps.remove(0) {{
            vidl::CapabilityWithDescription {{ capability: _, description: vidl::CapabilityDescription::Memory {{ ptr, len, permissions: vidl::internal::MemoryPermissions::READ_WRITE }} }} => {{
                let deserializer = vidl::materialize::Deserializer::new(unsafe {{ core::slice::from_raw_parts(ptr, len) }}, &caps);
                deserializer.deserialize().expect("deserialize success")
            }}
            _ => panic!("First cap in response not memory!"),
        }}  
    }}
    
"#, service.name.to_uppercase(), method.name.to_uppercase()));

        Ok(())
    }

    fn lower_service_server_async(&self, compiled: &mut CompiledVidl, service: &Service) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!(
            "pub trait Async{}Provider {{
    type Error;",
            service.name
        ));
        for method in &service.methods {
            self.lower_method_server_async(compiled, method)?;
        }
        compiled.write_str("\n}\n\n");
        compiled.write_fmt(format_args!(r#"pub struct Async{0}<T: Async{0}Provider>(T, vidl::present::IpcChannel);

impl<T: Async{0}Provider> Async{0}<T> {{
    pub fn new(provider: T, channel: vidl::CapabilityPtr) -> Self {{ Self(provider, vidl::present::IpcChannel::new(channel)) }}
    pub async fn serve(&mut self) -> ! {{
        loop {{
            let Ok((msg, mut caps)) = self.1.read_with_all_caps().await else {{ continue }};
            if caps.is_empty() {{
                // Need at least one cap for the RPC message
                continue;
            }}

            // If its not a memory cap we got a problem
            let buffer = {{
                let vidl::CapabilityWithDescription {{
                    capability: _,
                    description: vidl::CapabilityDescription::Memory {{ ptr, len, permissions: vidl::internal::MemoryPermissions::READ_WRITE }},
                }} = caps.remove(0) else {{ continue }};
                unsafe {{ core::slice::from_raw_parts(ptr, len) }}
            }};

            match msg.0[0] {{
"#, service.name));

        for method in &service.methods {
            compiled.write_fmt(format_args!(
                r#"                {}_{}_ID => {{
                let deserializer = vidl::materialize::Deserializer::new(buffer, &caps[..]);
                let Ok(("#,
                service.name.to_uppercase(),
                method.name.to_uppercase()
            ));
            for arg in &method.arguments {
                compiled.write_fmt(format_args!("{},", arg.0));
            }
            compiled.write_str(")) = deserializer.deserialize::<(");
            for arg in &method.arguments {
                self.lower_type(compiled, &arg.1, true)?;
                compiled.write_str(", ");
            }
            compiled.write_str(")>() else { continue };\n");
            compiled.write_fmt(format_args!("                if let Ok(response) = self.0.{}(", method.name));
            for (i, arg) in method.arguments.iter().enumerate() {
                compiled.write_fmt(format_args!("{}", arg.0));
                if i + 1 != method.arguments.len() {
                    compiled.write_str(", ");
                }
            }
            compiled.write_fmt(format_args!(
                r#").await {{
                    let mut serializer = vidl::materialize::Serializer::new();
                    serializer.serialize(&response).unwrap();
                    let (buffer, mut caps) = serializer.into_parts();
                    let mut mem = vidl::SharedMemoryAllocation::public_rw(vidl::Bytes(buffer.len())).unwrap();
                    unsafe {{ mem.as_mut()[..buffer.len()].copy_from_slice(&buffer) }};
                    caps.insert(0, vidl::Capability {{ cptr: mem.cptr, rights: vidl::CapabilityRights::READ }});
                    let _ = self.1.send(vidl::EndpointMessage([{}_{}_ID, 0, 0, 0, 0, 0, 0]), &caps[..]);
                }}"#,
                service.name.to_uppercase(),
                method.name.to_uppercase()
            ));

            compiled.write_str("            },\n");
        }

        compiled.write_str(
            r#"                _ => {},
            }
        }
    }
}
        "#,
        );

        Ok(())
    }

    fn lower_method_server_async(&self, compiled: &mut CompiledVidl, method: &Method) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!("\n    async fn {}(&mut self, ", method.name));

        for (i, arg) in method.arguments.iter().enumerate() {
            compiled.write_fmt(format_args!("{}: ", arg.0));
            self.lower_type(compiled, &arg.1, true)?;
            if i + 1 != method.arguments.len() {
                compiled.write_str(", ");
            }
        }

        compiled.write_str(")");

        match &method.return_type {
            Some(ret_type) => {
                compiled.write_str(" -> Result<");
                self.lower_type(compiled, ret_type, true)?;
                compiled.write_str(", Self::Error>")
            }
            None => compiled.write_str(" -> Result<(), Self::Error>"),
        }

        compiled.write_str(";");

        Ok(())
    }

    fn lower_service_client_async(&self, compiled: &mut CompiledVidl, service: &Service) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!(
            r"pub struct Async{0}Client(vidl::present::IpcChannel);

impl Async{0}Client {{

",
            service.name
        ));

        compiled.write_str(
            r"    pub fn new(cptr: std::ipc::CapabilityPtr) -> Self { Self(vidl::present::IpcChannel::new(cptr)) }

",
        );

        for method in &service.methods {
            self.lower_method_client_async(compiled, method, service)?;
        }
        compiled.write_str("\n}\n\n");

        Ok(())
    }

    fn lower_method_client_async(
        &self,
        compiled: &mut CompiledVidl,
        method: &Method,
        service: &Service,
    ) -> Result<(), CompileError> {
        compiled.write_fmt(format_args!("    pub async fn {}(&self, ", method.name));
        for (i, arg) in method.arguments.iter().enumerate() {
            compiled.write_fmt(format_args!("{}: ", arg.0));
            self.lower_type(compiled, &arg.1, false)?;
            if i + 1 != method.arguments.len() {
                compiled.write_str(",");
            }
        }

        compiled.write_str(")");

        if let Some(ret_type) = &method.return_type {
            compiled.write_str(" ->");
            self.lower_type(compiled, ret_type, true)?;
        }

        compiled.write_str(
            r" {
        let mut serializer = vidl::materialize::Serializer::new();
        serializer.serialize(&(",
        );

        for arg in &method.arguments {
            compiled.write_fmt(format_args!("&{},", arg.0));
        }

        compiled.write_fmt(format_args!(r#")).unwrap();
        let (buffer, mut caps) = serializer.into_parts();
        let mut mem = vidl::SharedMemoryAllocation::public_rw(vidl::Bytes(buffer.len())).unwrap();
        unsafe {{ mem.as_mut()[..buffer.len()].copy_from_slice(&buffer) }};
        caps.insert(0, vidl::Capability {{ cptr: mem.cptr, rights: vidl::CapabilityRights::READ }});
        self.0.send(vidl::EndpointMessage([{}_{}_ID, 0, 0, 0, 0, 0, 0]), &caps[..]).unwrap();
        let (_msg, mut caps) = self.0.read_with_all_caps().await.unwrap();

        match caps.remove(0) {{
            vidl::CapabilityWithDescription {{ capability: _, description: vidl::CapabilityDescription::Memory {{ ptr, len, permissions: vidl::internal::MemoryPermissions::READ_WRITE }} }} => {{
                let deserializer = vidl::materialize::Deserializer::new(unsafe {{ core::slice::from_raw_parts(ptr, len) }}, &caps);
                deserializer.deserialize().expect("deserialize success")
            }}
            _ => panic!("First cap in response not memory!"),
        }}  
    }}
    
"#, service.name.to_uppercase(), method.name.to_uppercase()));

        Ok(())
    }

    fn lower_type(&self, compiled: &mut CompiledVidl, ty: &Type, in_return_position: bool) -> Result<(), CompileError> {
        match ty {
            Type::Path { path, generics } => {
                // TODO: check for import vs defined in scope here
                let path = match self.usages.get(path.first().unwrap()) {
                    None => match self.providers.get(path.first().unwrap()) {
                        Some(provider) => {
                            alloc::format!("{}::{}", provider, path.get(1..).unwrap_or_default().join("::"))
                        }
                        None => path.join("::"),
                    },
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
            Type::Str => match in_return_position {
                true => compiled.write_str("vidl::core::String"),
                false => compiled.write_str("&vidl::core::Str"),
            },
            Type::Array(ty, size) => {
                compiled.write_str("[");
                self.lower_type(compiled, ty, in_return_position)?;
                compiled.write_fmt(format_args!("; {}]", size));
            }
        }

        Ok(())
    }

    fn lower_typedef(
        &self,
        compiled: &mut CompiledVidl,
        attributes: &[String],
        typedef: &TypeDefinition,
    ) -> Result<(), CompileError> {
        match typedef {
            TypeDefinition::Struct(strukt) => self.lower_struct(compiled, attributes, strukt),
            TypeDefinition::Enum(enoom) => self.lower_enum(compiled, attributes, enoom),
        }
    }

    fn lower_struct(
        &self,
        compiled: &mut CompiledVidl,
        attributes: &[String],
        strukt: &Struct,
    ) -> Result<(), CompileError> {
        let extra_traits = self.attributes_to_traits(attributes);
        compiled.write_fmt(format_args!(
            r#"#[derive(Debug, vidl::materialize::Deserialize, vidl::materialize::Serializable, vidl::materialize::Serialize{})]
#[materialize(reexport_path = "vidl::materialize")]
pub struct {}"#,
            extra_traits,
            strukt.name
        ));

        if let Some(generics) = &strukt.generics {
            compiled.write_str("<");
            generics.iter().enumerate().for_each(|(i, ty)| {
                compiled.write_str(ty);
                if i + 1 != generics.len() {
                    compiled.write_str(", ");
                }
            });
            compiled.write_str(">");
        }

        compiled.write_str(" {\n");
        for field in &strukt.fields {
            compiled.write_fmt(format_args!("    pub {}: ", field.name));
            self.lower_type(compiled, &field.ty, true)?;
            compiled.write_str(",\n");
        }
        compiled.write_str("}\n\n");
        Ok(())
    }

    fn lower_enum(&self, compiled: &mut CompiledVidl, attributes: &[String], enoom: &Enum) -> Result<(), CompileError> {
        let extra_traits = self.attributes_to_traits(attributes);
        compiled.write_fmt(format_args!(
            r#"#[derive(Debug, vidl::materialize::Deserialize, vidl::materialize::Serializable, vidl::materialize::Serialize{})]
#[materialize(reexport_path = "vidl::materialize")]
pub enum {}"#,
            extra_traits,
            enoom.name
        ));

        if let Some(generics) = &enoom.generics {
            compiled.write_str("<");
            generics.iter().enumerate().for_each(|(i, ty)| {
                compiled.write_str(ty);
                if i + 1 != generics.len() {
                    compiled.write_str(", ");
                }
            });
            compiled.write_str(">");
        }

        compiled.write_str(" {\n");
        for variant in &enoom.variants {
            compiled.write_fmt(format_args!("    {}", variant.name));
            if let Some(associated_data) = &variant.associated_data {
                match associated_data {
                    parser::VariantData::Struct(fields) => {
                        compiled.write_str("{\n");
                        for field in fields {
                            compiled.write_fmt(format_args!("        {}: ", field.name));
                            self.lower_type(compiled, &field.ty, true)?;
                            compiled.write_str(",\n");
                        }
                        compiled.write_str("    }");
                    }
                    parser::VariantData::Tuple(tys) => {
                        compiled.write_str("(");
                        for (i, ty) in tys.iter().enumerate() {
                            self.lower_type(compiled, ty, true)?;
                            if i + 1 != tys.len() {
                                compiled.write_str(", ");
                            }
                        }
                        compiled.write_str(")");
                    }
                }
            }
            compiled.write_str(",\n");
        }
        compiled.write_str("}\n\n");
        Ok(())
    }

    fn attributes_to_traits(&self, attributes: &[String]) -> String {
        let mut traits = String::new();

        for attribute in attributes {
            match &**attribute {
                "trivial" => traits.push_str("Clone, Copy, "),
                "comparable" => traits.push_str("PartialEq, Eq, "),
                "orderable" => traits.push_str("PartialEq, Eq, PartialOrd, Ord, "),
                _ => {}
            }
        }

        if !traits.is_empty() {
            traits.insert_str(0, ", ");
            traits.pop();
            traits.pop();
        }

        traits
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
