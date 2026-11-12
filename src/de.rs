use std::collections::hash_map;

use serde::de::{self, Deserializer, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};

use crate::ast::Value;
use crate::Error;

// Макрос объявлен на уровне модуля (не внутри impl!)
// при раскрытии внутри impl `'de`, `V`, `Visitor`, `Error`, `Value` уже в области видимости.
macro_rules! deserialize_int {
    ($method:ident, $ty:ty, $visit:ident) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            match self {
                Value::Number(n)
                    if n.fract() == 0.0
                        && n >= <$ty>::MIN as f64
                        && n <= <$ty>::MAX as f64 =>
                {
                    visitor.$visit(n as $ty)
                }
                Value::Number(n) => Err(Error::new(format!(
                    "number {} out of range for {}",
                    n,
                    stringify!($ty)
                ))),
                other => Err(Error::new(format!(
                    "expected number ({}), got {:?}",
                    stringify!($ty),
                    other
                ))),
            }
        }
    };
}

impl<'de> Deserializer<'de> for Value {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Bool(b) => visitor.visit_bool(b),
            Value::Number(n) => visitor.visit_f64(n),
            Value::String(s) => visitor.visit_string(s),
            Value::Array(v) => visitor.visit_seq(Seq { iter: v.into_iter() }),
            Value::Object(m) => visitor.visit_map(Map {
                iter: m.into_iter(),
                value: None,
            }),
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::String(s) => visitor.visit_string(s),
            // В конфиге типа `bind` числа встречаются в массиве строк:
            // [ $mod, "1", "scroll", -30 ] -> Vec<String>. Конвертируем.
            Value::Number(n) => visitor.visit_string(format_number(n)),
            Value::Bool(b) => visitor.visit_string(b.to_string()),
            other => Err(Error::new(format!("expected string, got {other:?}"))),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_enum(Enum { value: self })
    }

    deserialize_int!(deserialize_i8, i8, visit_i8);
    deserialize_int!(deserialize_i16, i16, visit_i16);
    deserialize_int!(deserialize_i32, i32, visit_i32);
    deserialize_int!(deserialize_i64, i64, visit_i64);
    deserialize_int!(deserialize_u8, u8, visit_u8);
    deserialize_int!(deserialize_u16, u16, visit_u16);
    deserialize_int!(deserialize_u32, u32, visit_u32);
    deserialize_int!(deserialize_u64, u64, visit_u64);

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Number(n) => visitor.visit_f32(n as f32),
            other => Err(Error::new(format!("expected f32, got {other:?}"))),
        }
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self {
            Value::Number(n) => visitor.visit_f64(n),
            other => Err(Error::new(format!("expected f64, got {other:?}"))),
        }
    }

    serde::forward_to_deserialize_any! {
        bool char str bytes byte_buf unit unit_struct seq tuple
        tuple_struct map struct identifier ignored_any
    }
}

pub(crate) fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

struct Seq {
    iter: std::vec::IntoIter<Value>,
}

impl<'de> SeqAccess<'de> for Seq {
    type Error = Error;

    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        match self.iter.next() {
            Some(v) => seed.deserialize(v).map(Some),
            None => Ok(None),
        }
    }
}

struct Map {
    iter: hash_map::IntoIter<String, Value>,
    value: Option<Value>,
}

impl<'de> MapAccess<'de> for Map {
    type Error = Error;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.iter.next() {
            Some((k, v)) => {
                self.value = Some(v);
                seed.deserialize(Value::String(k)).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Error> {
        let v = self
            .value
            .take()
            .ok_or_else(|| Error::new("value missing"))?;
        seed.deserialize(v)
    }
}

struct Enum {
    value: Value,
}

impl<'de> EnumAccess<'de> for Enum {
    type Error = Error;
    type Variant = Variant;

    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let (name, value) = match self.value {
            Value::String(s) => (s, None),
            Value::Object(mut m) if m.len() == 1 => {
                let key = m.keys().next().unwrap().clone();
                let val = m.remove(&key).unwrap();
                (key, Some(val))
            }
            other => {
                return Err(Error::new(format!(
                    "expected enum variant (string or single-key object), got {other:?}"
                )))
            }
        };
        Ok((seed.deserialize(Value::String(name))?, Variant { value }))
    }
}

struct Variant {
    value: Option<Value>,
}

impl<'de> VariantAccess<'de> for Variant {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.value {
            None => Ok(()),
            Some(_) => Err(Error::new("expected unit variant")),
        }
    }

    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Error> {
        seed.deserialize(
            self.value
                .ok_or_else(|| Error::new("expected newtype variant"))?,
        )
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.value {
            Some(Value::Array(v)) => visitor.visit_seq(Seq {
                iter: v.into_iter(),
            }),
            _ => Err(Error::new("expected tuple variant")),
        }
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.value {
            Some(Value::Object(m)) => visitor.visit_map(Map {
                iter: m.into_iter(),
                value: None,
            }),
            _ => Err(Error::new("expected struct variant")),
        }
    }
}