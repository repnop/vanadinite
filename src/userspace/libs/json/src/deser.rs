use crate::parser::ParseError;

pub trait Serializer: Sized {
    fn write(&mut self, byte: u8);
    fn write_all(&mut self, bytes: &[u8]) {
        for byte in bytes.iter().copied() {
            self.write(byte);
        }
    }

    fn serialize_object<'a, I>(&mut self, members: I)
    where
        I: Iterator<Item = (&'a str, &'a dyn Serialize<Self>)>,
        Self: 'a,
    {
        self.write(b'{');
        for (name, value) in members {
            self.write(b'"');
            self.write_all(name.as_bytes());
            self.write_all(&[b'"', b':']);
            value.serialize(self);
            self.write(b',');
        }
        self.write(b'}');
    }

    fn serialize_number(&mut self, n: i64) {
        // FIXME: this is lazy but easy
        self.write_all(alloc::format!("{}", n).as_bytes());
    }

    fn serialize_list<'a, F>(&mut self, values: F)
    where
        F: Iterator<Item = &'a dyn Serialize<Self>> + 'a,
        Self: 'a,
    {
        self.write(b'[');
        for value in values {
            value.serialize(self);
            self.write(b',');
        }
        self.write(b']');
    }

    fn serialize_string(&mut self, s: &str) {
        self.write(b'"');
        self.write_all(s.as_bytes());
        self.write(b'"');
    }

    fn serialize_null(&mut self) {
        self.write_all(b"null");
    }
}

impl Serializer for &mut [u8] {
    fn write(&mut self, byte: u8) {
        let (a, b) = core::mem::take(self).split_at_mut(1);
        a[0] = byte;
        *self = b;
    }

    fn write_all(&mut self, bytes: &[u8]) {
        let amt = core::cmp::min(bytes.len(), self.len());
        let (a, b) = core::mem::take(self).split_at_mut(amt);
        a.copy_from_slice(&bytes[..amt]);
        *self = b;
    }
}

impl Serializer for alloc::vec::Vec<u8> {
    fn write(&mut self, byte: u8) {
        self.push(byte);
    }

    fn write_all(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl<S: Serializer> Serializer for &'_ mut S {
    fn write(&mut self, byte: u8) {
        (*self).write(byte);
    }

    fn write_all(&mut self, bytes: &[u8]) {
        (*self).write_all(bytes);
    }
}

pub trait Serialize<S: Serializer> {
    fn serialize(&self, serializer: &mut S);
}

impl<S: Serializer> Serialize<S> for alloc::string::String {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_string(self);
    }
}

impl<S: Serializer> Serialize<S> for str {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_string(self);
    }
}

impl<S: Serializer> Serialize<S> for i64 {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(*self);
    }
}

impl<S: Serializer> Serialize<S> for i32 {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(*self as _);
    }
}

impl<S: Serializer> Serialize<S> for i16 {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(*self as _);
    }
}

impl<S: Serializer> Serialize<S> for i8 {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(*self as _);
    }
}

impl<S: Serializer> Serialize<S> for isize {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(*self as _);
    }
}

impl<S: Serializer> Serialize<S> for u64 {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(i64::try_from(*self).unwrap());
    }
}

impl<S: Serializer> Serialize<S> for u32 {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(i64::try_from(*self).unwrap());
    }
}

impl<S: Serializer> Serialize<S> for u16 {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(i64::try_from(*self).unwrap());
    }
}

impl<S: Serializer> Serialize<S> for u8 {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(i64::try_from(*self).unwrap());
    }
}

impl<S: Serializer> Serialize<S> for usize {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_number(i64::try_from(*self).unwrap());
    }
}

impl<S: Serializer, T: Serialize<S>> Serialize<S> for alloc::vec::Vec<T> {
    fn serialize(&self, serializer: &mut S) {
        serializer.serialize_list(self.iter().map(|t| t as &dyn Serialize<S>))
    }
}

use crate::parser;
pub trait Deserializer<'a> {
    fn deserialize_value(&mut self) -> Result<crate::Value, ParseError>;
    fn deserialize_object<F>(&mut self, member_callback: F) -> Result<(), ParseError>
    where
        F: FnMut(&str, &mut Self) -> Result<(), ParseError>;
    fn deserialize_list<F>(&mut self, item_callback: F) -> Result<(), ParseError>
    where
        F: FnMut(&mut Self) -> Result<(), ParseError>;
    fn deserialize_number(&mut self) -> Result<i64, ParseError>;
    fn deserialize_null(&mut self) -> Result<(), ParseError>;
    fn deserialize_str(&mut self) -> Result<&'a str, ParseError>;
}

impl<'a> Deserializer<'a> for crate::parser::Parser<'a> {
    fn deserialize_value(&mut self) -> Result<crate::Value, ParseError> {
        self.parse::<crate::Value>()
    }

    fn deserialize_object<F>(&mut self, mut member_callback: F) -> Result<(), ParseError>
    where
        F: FnMut(&str, &mut Self) -> Result<(), ParseError>,
    {
        self.parse::<parser::LeftBrace>()?;

        while let Some((name, _)) = self.parse_or_rewind::<(&str, parser::Colon)>() {
            member_callback(name, self)?;

            if self.parse::<Option<parser::Comma>>()?.is_none() {
                break;
            }
        }

        self.parse::<parser::RightBrace>()?;

        Ok(())
    }

    fn deserialize_list<F>(&mut self, mut item_callback: F) -> Result<(), ParseError>
    where
        F: FnMut(&mut Self) -> Result<(), ParseError>
    {
        self.parse::<parser::LeftBracket>().unwrap();

        while self.peek() != Some(']') {
            item_callback(self)?;

            if self.parse::<Option<parser::Comma>>()?.is_none() {
                break;
            }
        }

        self.parse::<parser::RightBracket>()?;

        Ok(())
    }

    fn deserialize_number(&mut self) -> Result<i64, ParseError> {
        self.parse::<i64>()
    }

    fn deserialize_null(&mut self) -> Result<(), ParseError> {
        self.skip_whitespace();
        self.eat('n')?;
        self.eat('u')?;
        self.eat('l')?;
        self.eat('l')?;

        Ok(())
    }

    #[track_caller]
    fn deserialize_str(&mut self) -> Result<&'a str, ParseError> {
        self.parse::<&str>()
    }
}

pub trait Deserialize: Sized {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError>;
}

impl Deserialize for alloc::string::String {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(deserializer.deserialize_str()?.into())
    }
}

impl Deserialize for i64 {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        deserializer.deserialize_number()
    }
}

impl Deserialize for i32 {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(i32::try_from(deserializer.deserialize_number()?)?)
    }
}

impl Deserialize for i16 {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(Self::try_from(deserializer.deserialize_number()?)?)
    }
}

impl Deserialize for i8 {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(Self::try_from(deserializer.deserialize_number()?)?)
    }
}

impl Deserialize for isize {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(Self::try_from(deserializer.deserialize_number()?)?)
    }
}

impl Deserialize for u64 {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(Self::try_from(deserializer.deserialize_number()?)?)
    }
}

impl Deserialize for u32 {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(Self::try_from(deserializer.deserialize_number()?)?)
    }
}

impl Deserialize for u16 {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(Self::try_from(deserializer.deserialize_number()?)?)
    }
}

impl Deserialize for u8 {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(Self::try_from(deserializer.deserialize_number()?)?)
    }
}

impl Deserialize for usize {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        Ok(Self::try_from(deserializer.deserialize_number()?)?)
    }
}

impl<T: Deserialize> Deserialize for alloc::vec::Vec<T> {
    fn deserialize<'a, D: Deserializer<'a>>(deserializer: &mut D) -> Result<Self, ParseError> {
        let mut this = alloc::vec::Vec::new();
        deserializer.deserialize_list(|deserializer| Ok(this.push(T::deserialize(deserializer)?)))?;
        Ok(this)
    }
}
