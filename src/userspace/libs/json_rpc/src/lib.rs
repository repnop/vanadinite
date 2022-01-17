#![no_std]

use json::deser::{Deserialize, Serialize, Serializer};

extern crate alloc;

#[macro_export]
macro_rules! rpc {
    ($service:ident, { $(fn $f:ident($($arg:ident: $t:ty),*)? $(-> $ret:ty)?);+ }) => {
        trait $service {
            $(fn $f:ident($($arg:ident: $t:ty),*)? $(-> $ret:ty)?);+
        }
    };
}

json::derive! {
    struct Request<T> {
        method: alloc::string::String,
        params: Option<T>,
        id: Option<i64>,
    }
}

json::derive! {
    struct Response<T, E> {
        method: alloc::string::String,
        result: CallResult<T, E>,
        id: Option<i64>,
    }
}

enum CallResult<T, E> {
    Ok(T),
    Err(E),
}

impl<S: Serializer, T: Serialize<S>, E: Serialize<S>> Serialize<S> for CallResult<T, E> {
    fn serialize(&self, serializer: &mut S) {
        match self {
            Self::Ok(t) => serializer.serialize_object(core::iter::once(("ok", t as &dyn Serialize<S>))),
            Self::Err(e) => serializer.serialize_object(core::iter::once(("err", e as &dyn Serialize<S>))),
        }
    }
}

impl<T: Deserialize, E: Deserialize> Deserialize for CallResult<T, E> {
    fn deserialize<'a, D: json::deser::Deserializer<'a>>(
        deserializer: &mut D,
    ) -> Result<Self, json::deser::DeserializeError> {
        json::derive! {
            #[allow(non_snake_case)]
            struct OkVariant<U> {
                ok: U,
            }
        }

        json::derive! {
            #[allow(non_snake_case)]
            struct ErrVariant<U> {
                err: U,
            }
        }

        deserializer
            .try_deserialize::<OkVariant<T>>()
            .map(|k| Self::Ok(k.ok))
            .or_else(|| deserializer.try_deserialize::<ErrVariant<E>>().map(|e| Self::Err(e.err)))
            .ok_or(json::deser::DeserializeError::UnknownVariantValue)
    }
}
