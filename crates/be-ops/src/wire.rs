use std::{collections::BTreeMap, convert::Infallible, fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{DeserializeOwned, MapAccess, Visitor},
};

pub trait Encode<T> {
    fn serialize<S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error>;
}

pub trait Decode<T> {
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error>;
}

pub enum Plain {}

pub enum Bytes {}

pub struct List<S>(Infallible, PhantomData<S>);

pub struct Optional<S>(Infallible, PhantomData<S>);

pub struct Fallible<S, E>(Infallible, PhantomData<(S, E)>);

pub struct Map<K, V>(Infallible, PhantomData<(K, V)>);

impl<T: Serialize> Encode<T> for Plain {
    fn serialize<S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
        value.serialize(serializer)
    }
}

impl<T: DeserializeOwned> Decode<T> for Plain {
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<T, D::Error> {
        T::deserialize(deserializer)
    }
}

impl Encode<Vec<u8>> for Bytes {
    fn serialize<S: Serializer>(value: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(value)
    }
}

impl Decode<Vec<u8>> for Bytes {
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        serde_bytes::ByteBuf::deserialize(deserializer).map(serde_bytes::ByteBuf::into_vec)
    }
}

impl<T, S: Encode<T>> Encode<Vec<T>> for List<S> {
    fn serialize<Z: Serializer>(value: &Vec<T>, serializer: Z) -> Result<Z::Ok, Z::Error> {
        serializer.collect_seq(value.iter().map(Encoded::<S, T>::new))
    }
}

impl<T, S: Decode<T>> Decode<Vec<T>> for List<S> {
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<T>, D::Error> {
        let items = Vec::<Decoded<S, T>>::deserialize(deserializer)?;
        Ok(items.into_iter().map(|item| item.0).collect())
    }
}

impl<T, S: Encode<T>> Encode<Option<T>> for Optional<S> {
    fn serialize<Z: Serializer>(value: &Option<T>, serializer: Z) -> Result<Z::Ok, Z::Error> {
        match value {
            Some(value) => serializer.serialize_some(&Encoded::<S, T>::new(value)),
            None => serializer.serialize_none(),
        }
    }
}

impl<T, S: Decode<T>> Decode<Option<T>> for Optional<S> {
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<T>, D::Error> {
        let value = Option::<Decoded<S, T>>::deserialize(deserializer)?;
        Ok(value.map(|value| value.0))
    }
}

#[derive(Deserialize)]
#[serde(rename = "Result")]
enum Outcome<T, E> {
    Ok(T),
    Err(E),
}

impl<T, E, S: Encode<T>, F: Encode<E>> Encode<Result<T, E>> for Fallible<S, F> {
    fn serialize<Z: Serializer>(value: &Result<T, E>, serializer: Z) -> Result<Z::Ok, Z::Error> {
        match value {
            Ok(value) => serializer.serialize_newtype_variant(
                "Result",
                0,
                "Ok",
                &Encoded::<S, T>::new(value),
            ),
            Err(error) => serializer.serialize_newtype_variant(
                "Result",
                1,
                "Err",
                &Encoded::<F, E>::new(error),
            ),
        }
    }
}

impl<T, E, S: Decode<T>, F: Decode<E>> Decode<Result<T, E>> for Fallible<S, F> {
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Result<T, E>, D::Error> {
        Ok(
            match Outcome::<Decoded<S, T>, Decoded<F, E>>::deserialize(deserializer)? {
                Outcome::Ok(value) => Ok(value.0),
                Outcome::Err(error) => Err(error.0),
            },
        )
    }
}

impl<K, V, KS: Encode<K>, VS: Encode<V>> Encode<BTreeMap<K, V>> for Map<KS, VS> {
    fn serialize<Z: Serializer>(value: &BTreeMap<K, V>, serializer: Z) -> Result<Z::Ok, Z::Error> {
        serializer.collect_map(
            value
                .iter()
                .map(|(key, value)| (Encoded::<KS, K>::new(key), Encoded::<VS, V>::new(value))),
        )
    }
}

impl<K: Ord, V, KS: Decode<K>, VS: Decode<V>> Decode<BTreeMap<K, V>> for Map<KS, VS> {
    fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error> {
        struct Entries<K, V, KS, VS>(PhantomData<fn() -> (K, V, KS, VS)>);

        impl<'de, K: Ord, V, KS: Decode<K>, VS: Decode<V>> Visitor<'de> for Entries<K, V, KS, VS> {
            type Value = BTreeMap<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut map = BTreeMap::new();
                while let Some((key, value)) =
                    access.next_entry::<Decoded<KS, K>, Decoded<VS, V>>()?
                {
                    map.insert(key.0, value.0);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(Entries::<K, V, KS, VS>(PhantomData))
    }
}

macro_rules! tuples {
    ($(($($index:tt $value:ident $shape:ident),+))*) => {$(
        impl<$($value, $shape: Encode<$value>),+> Encode<($($value,)+)> for ($($shape,)+) {
            fn serialize<Z: Serializer>(
                value: &($($value,)+),
                serializer: Z,
            ) -> Result<Z::Ok, Z::Error> {
                ($(Encoded::<$shape, $value>::new(&value.$index),)+).serialize(serializer)
            }
        }

        impl<$($value, $shape: Decode<$value>),+> Decode<($($value,)+)> for ($($shape,)+) {
            fn deserialize<'de, D: Deserializer<'de>>(
                deserializer: D,
            ) -> Result<($($value,)+), D::Error> {
                let value = <($(Decoded<$shape, $value>,)+)>::deserialize(deserializer)?;
                Ok(($(value.$index.0,)+))
            }
        }
    )*};
}

tuples! {
    (0 T0 S0)
    (0 T0 S0, 1 T1 S1)
    (0 T0 S0, 1 T1 S1, 2 T2 S2)
    (0 T0 S0, 1 T1 S1, 2 T2 S2, 3 T3 S3)
    (0 T0 S0, 1 T1 S1, 2 T2 S2, 3 T3 S3, 4 T4 S4)
    (0 T0 S0, 1 T1 S1, 2 T2 S2, 3 T3 S3, 4 T4 S4, 5 T5 S5)
    (0 T0 S0, 1 T1 S1, 2 T2 S2, 3 T3 S3, 4 T4 S4, 5 T5 S5, 6 T6 S6)
    (0 T0 S0, 1 T1 S1, 2 T2 S2, 3 T3 S3, 4 T4 S4, 5 T5 S5, 6 T6 S6, 7 T7 S7)
}

pub struct Encoded<'a, S, T>(&'a T, PhantomData<fn() -> S>);

impl<'a, S: Encode<T>, T> Encoded<'a, S, T> {
    pub fn new(value: &'a T) -> Self {
        Self(value, PhantomData)
    }
}

impl<S: Encode<T>, T> Serialize for Encoded<'_, S, T> {
    fn serialize<Z: Serializer>(&self, serializer: Z) -> Result<Z::Ok, Z::Error> {
        S::serialize(self.0, serializer)
    }
}

pub struct Decoded<S, T>(pub T, PhantomData<fn() -> S>);

impl<'de, S: Decode<T>, T> Deserialize<'de> for Decoded<S, T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        S::deserialize(deserializer).map(|value| Self(value, PhantomData))
    }
}

pub fn serialize<S: Encode<T>, T, Z: Serializer>(
    value: &T,
    serializer: Z,
) -> Result<Z::Ok, Z::Error> {
    S::serialize(value, serializer)
}

pub fn deserialize<'de, S: Decode<T>, T, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    S::deserialize(deserializer)
}

pub fn encode<S: Encode<T>, T>(value: &T) -> Vec<u8> {
    crate::encode(&Encoded::<S, T>::new(value))
}

pub fn decode<S: Decode<T>, T>(bytes: &[u8]) -> Option<T> {
    crate::decode::<Decoded<S, T>>(bytes).map(|value| value.0)
}
