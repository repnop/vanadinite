// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

extern crate alloc;

pub mod deser;
pub mod parser;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::ops::{Deref, Index};

#[macro_export]
macro_rules! derive {
    ($(#[$($attr:meta),+])? struct $name:ident$(<$($g:ident),+$(,)?>)? { $($field:ident: $t:ty),+ $(,)? }) => {
        $(#[$($attr),+])?
        struct $name$(<$($g),+>)? {
            $($field: $t),+
        }

        $crate::derive!(@deser struct $name$(<$($g),+>)? { $($field: $t),+ });
        $crate::derive!(@ser struct $name$(<$($g),+>)? { $($field: $t),+ });
    };

    (Serialize, $(#[$($attr:meta),+])? struct $name:ident$(<$($g:ident),+$(,)?>)? { $($field:ident: $t:ty),+ $(,)? }) => {
        $(#[$($attr),+])?
        struct $name$(<$($g),+>)? {
            $($field: $t),+
        }

        $crate::derive!(@ser struct $name$(<$($g),+>)? { $($field: $t),+ });
    };

    (Deserialize, $(#[$($attr:meta),+])? struct $name:ident$(<$($g:ident),+$(,)?>)? { $($field:ident: $t:ty),+ $(,)? }) => {
        $(#[$($attr),+])?
        struct $name$(<$($g),+>)? {
            $($field: $t),+
        }

        $crate::derive!(@deser struct $name$(<$($g),+>)? { $($field: $t),+ });
    };

    (@deser struct $name:ident$(<$($g:ident),+$(,)?>)? { $($field:ident: $t:ty),+ $(,)? }) => {
        impl$(<$($g),+>)? $crate::deser::Deserialize for $name$(<$($g),+>)?
        where
            $($($g: $crate::deser::Deserialize),+)?
        {
            fn deserialize<'a, D: $crate::deser::Deserializer<'a>>(deserializer: &mut D) -> Result<Self, $crate::deser::DeserializeError> {
                $(
                    let mut $field = <$t>::init();
                )+

                deserializer.deserialize_object(|name, deserializer| {
                    Ok(match name {
                        $(
                            stringify!($field) => $field = Some(<$t>::deserialize(deserializer)?),
                        )+
                        _ => core::mem::drop(deserializer.deserialize_value()?),
                    })
                })?;


                Ok(Self {
                    $($field: $field.ok_or($crate::deser::DeserializeError::MissingField(stringify!($field)))?),+
                })
            }
        }
    };

    (@ser struct $name:ident$(<$($g:ident),+$(,)?>)? { $($field:ident: $t:ty),+ $(,)? }) => {
        impl<S: $crate::deser::Serializer, $($($g),+)?> $crate::deser::Serialize<S> for $name$(<$($g),+>)?
        where
            $($($g: $crate::deser::Serialize<S>),+)?
        {
            fn serialize(&self, serializer: &mut S) {
                let members = &[
                    $((stringify!($field), &self.$field as &dyn $crate::deser::Serialize<S>)),+
                ];
                serializer.serialize_object(members.iter().copied());
            }
        }
    };

    // ($(#[$($attr:meta),+])? enum $name:ident$(<$($g:ident),+$(,)?>)? { $($variant:ident$(($t:ty))?),+ $(,)? }) => {
    //     $(#[$($attr),+])?
    //     enum $name$(<$($g),+>)? {
    //         $($variant$(($t))?),+
    //     }
    //
    //     $crate::derive!(@deser enum $name$(<$($g),+>)? { $($variant$(($t))?),+ });
    //     $crate::derive!(@ser enum $name$(<$($g),+>)? { $($variant$(($t))?),+ });
    // };
    //
    // (@deser enum $name:ident$(<$($g:ident),+$(,)?>)? { $($variant:ident$(($t:ty))?),+ $(,)? }) => {
    //     impl $crate::deser::Deserialize for $name
    //     where
    //         $($($g: $crate::deser::Deserialize),+)?
    //     {
    //         fn deserialize<'a, D: $crate::deser::Deserializer<'a>>(deserializer: &mut D) -> Result<Self, $crate::deser::DeserializeError> {
    //             todo!()
    //             // match deserializer.deserialize_str()? {
    //             //     $(s if s.eq_ignore_ascii_case(stringify!($variant)) => Ok(Self::$variant),)+
    //             //     s => Err($crate::deser::DeserializeError::UnknownVariantValue(s.into()))
    //             // }
    //         }
    //     }
    // };
    //
    // (@deser arm $deserializer:ident Self::$variant:ident($t:ty)) => {
    //     let mut val = <$t as deser::Deserialize>::init();
    //     deserializer.deserialize_object
    // };
    //
    //     (@ser enum $name:ident$(<$($g:ident),+$(,)?>)? { $($variant:ident$(($t:ty))?),+ $(,)? }) => {
    //     impl<S: $crate::deser::Serializer> $crate::deser::Serialize<S> for $name
    //     where
    //         $($($g: $crate::deser::Serialize<S>),+)?
    //     {
    //         fn serialize(&self, serializer: &mut S) {
    //             $(
    //                 $crate::derive!(@ser arm serializer self Self::$variant$(($t))?);
    //             )+
    //
    //             unreachable!();
    //         }
    //     }
    // };
    //
    // (@ser arm $serializer:ident $self:ident Self::$variant:ident($t:ty)) => {
    //     if let Self::$variant(val) = $self { return $serializer.serialize_object(core::iter::once((stringify!($variant), val as &dyn $crate::deser::Serialize<S>))); }
    // };
    //
    // (@ser arm $serializer:ident $self:ident Self::$variant:ident) => {
    //     if let Self::$variant = $self { return $serializer.serialize_string(stringify!($variant)); }
    // };
}

mod sealed {
    pub trait Sealed {}
}

pub trait ValueType: sealed::Sealed {
    fn try_from_value(value: &Value) -> Option<&Self>;
}

impl sealed::Sealed for Value {}
impl ValueType for Value {
    fn try_from_value(value: &Value) -> Option<&Self> {
        Some(value)
    }
}

impl sealed::Sealed for List {}
impl ValueType for List {
    fn try_from_value(value: &Value) -> Option<&Self> {
        match value {
            Value::List(list) => Some(list),
            _ => None,
        }
    }
}

impl sealed::Sealed for i64 {}
impl ValueType for i64 {
    fn try_from_value(value: &Value) -> Option<&Self> {
        match value {
            Value::Number(n) => Some(n),
            _ => None,
        }
    }
}

impl sealed::Sealed for Object {}
impl ValueType for Object {
    fn try_from_value(value: &Value) -> Option<&Self> {
        match value {
            Value::Object(object) => Some(object),
            _ => None,
        }
    }
}

impl sealed::Sealed for String {}
impl ValueType for String {
    fn try_from_value(value: &Value) -> Option<&Self> {
        match value {
            Value::String(string) => Some(string),
            _ => None,
        }
    }
}

impl sealed::Sealed for str {}
impl ValueType for str {
    fn try_from_value(value: &Value) -> Option<&Self> {
        match value {
            Value::String(string) => Some(&**string),
            _ => None,
        }
    }
}

impl sealed::Sealed for [Value] {}
impl ValueType for [Value] {
    fn try_from_value(value: &Value) -> Option<&Self> {
        match value {
            Value::List(list) => Some(&**list),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum Value {
    List(List),
    Number(i64),
    Object(Object),
    String(String),
    Bool(bool),
    Null,
}

impl<'a> parser::Parseable<'a> for Value {
    fn parse(parser: &mut parser::Parser<'a>) -> Result<Self, parser::ParseError> {
        match parser.peek().ok_or(parser::ParseError::UnexpectedEof)? {
            '"' => Ok(Self::String(parser.parse::<String>()?)),
            '[' => Ok(Self::List(parser.parse::<List>()?)),
            '{' => Ok(Self::Object(parser.parse::<Object>()?)),
            't' | 'f' => Ok(Self::Bool(parser.parse::<bool>()?)),
            c if c.is_ascii_alphanumeric() => Ok(Self::Number(parser.parse::<i64>()?)),
            c => Err(parser::ParseError::UnexpectedCharacter(c)),
        }
    }
}

impl Index<&'_ str> for Value {
    type Output = Value;

    #[track_caller]
    fn index(&self, index: &str) -> &Self::Output {
        match self {
            Self::Object(obj) => &obj[index],
            Self::List(_) => panic!("value is a list, not an object"),
            Self::Number(_) => panic!("value is a number, not an object"),
            Self::String(_) => panic!("value is a string, not an object"),
            Self::Bool(_) => panic!("value is a bool, not an object"),
            Self::Null => panic!("value is null, not an object"),
        }
    }
}

impl Index<usize> for Value {
    type Output = Value;

    #[track_caller]
    fn index(&self, index: usize) -> &Self::Output {
        match self {
            Self::List(list) => &list[index],
            Self::Object(_) => panic!("value us an object, not a list"),
            Self::Number(_) => panic!("value is a number, not a list"),
            Self::String(_) => panic!("value is a string, not a list"),
            Self::Bool(_) => panic!("value is a bool, not a list"),
            Self::Null => panic!("value is null, not a list"),
        }
    }
}

#[derive(Debug)]
pub struct Object {
    map: BTreeMap<String, Value>,
}

impl Object {
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.map.get(name)
    }

    pub fn get_as<T: ValueType + ?Sized>(&self, name: &str) -> Option<&T> {
        self.map.get(name).and_then(T::try_from_value)
    }

    pub fn remove(&mut self, name: &str) -> Option<Value> {
        self.map.remove(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.map.iter().map(|(k, v)| (&**k, v))
    }
}

impl Index<&'_ str> for Object {
    type Output = Value;

    #[track_caller]
    fn index(&self, index: &str) -> &Self::Output {
        match self.get(index) {
            Some(value) => value,
            None => panic!("no object member named {:?}", index),
        }
    }
}

#[rustfmt::skip]
impl<'a> parser::Parseable<'a> for Object {
    #[inline]
    fn parse(parser: &mut parser::Parser<'a>) -> Result<Self, parser::ParseError> {
        use parser::{Colon, Comma, LeftBrace, RightBrace, RepeatUntilNoTrail};

        let (_, values, _) = parser.parse::<(
            LeftBrace,
            RepeatUntilNoTrail<
                (String, Colon, Value),
                Comma,
            >,
            RightBrace,
        )>()?;

        Ok(Self { map: values.values.into_iter().map(|(name, _, value)| (name, value)).collect() })
    }
}

#[derive(Debug)]
pub struct List {
    values: Vec<Value>,
}

impl Deref for List {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

#[rustfmt::skip]
impl<'a> parser::Parseable<'a> for List {
    #[inline]
    fn parse(parser: &mut parser::Parser<'a>) -> Result<Self, parser::ParseError> {
        use parser::{Comma, LeftBracket, RightBracket, RepeatUntilNoTrail};

        let (_, values, _) = parser.parse::<(
            LeftBracket,
            RepeatUntilNoTrail<
                Value,
                Comma,
            >,
            RightBracket,
        )>()?;

        Ok(Self { values: values.values })
    }
}

pub fn to_bytes<S: deser::Serialize<Vec<u8>>>(data: &S) -> Vec<u8> {
    let mut v = Vec::new();
    serialize(&mut v, data);
    v
}

pub fn serialize<Sr: deser::Serializer, S: deser::Serialize<Sr>>(serializer: &mut Sr, data: &S) {
    data.serialize(serializer)
}

pub fn deserialize<D: deser::Deserialize>(bytes: &[u8]) -> Result<D, deser::DeserializeError> {
    D::deserialize(&mut parser::Parser::new(bytes))
}
