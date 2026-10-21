//! Typed wire values retain token shapes under additive serde_json features.
use serde::{Deserialize, Deserializer, de};
use serde_json::{Value, value::RawValue};
use std::{collections::BTreeMap, fmt, marker::PhantomData};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Hello {
    pub protocol: String,
    #[serde(deserialize_with = "unique_map")]
    pub bindings: BTreeMap<String, String>,
    #[serde(default)]
    pub manifest: Option<Box<RawValue>>,
}
fn unique_map<'de, D, T>(d: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Unique<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an object with unique field names")
        }
        fn visit_map<M: de::MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
            let mut values = BTreeMap::new();
            while let Some((name, value)) = map.next_entry::<String, T>()? {
                if values.insert(name.clone(), value).is_some() {
                    return Err(de::Error::custom(format!("duplicate field {name:?}")));
                }
            }
            Ok(values)
        }
    }
    d.deserialize_map(Unique(PhantomData))
}
/// Reconstruct metadata containers from raw tokens, so a real object bearing
/// serde_json's private number key remains an object under arbitrary_precision.
/// Borrowed raw children avoid copying a deeply nested source at every level.
pub(super) fn metadata(raw: Option<&RawValue>) -> Result<Value, String> {
    fn value(raw: &RawValue, depth: usize) -> Result<Value, String> {
        if depth >= 128 {
            return Err("worker manifest exceeds nesting limit 128".to_owned());
        }
        let text = raw.get();
        match text.as_bytes()[0] {
            b'{' => {
                let mut decoder = serde_json::Deserializer::from_str(text);
                let object = unique_map::<_, &RawValue>(&mut decoder).map_err(|e| e.to_string())?;
                object
                    .into_iter()
                    .map(|(k, v)| Ok((k, value(v, depth + 1)?)))
                    .collect::<Result<serde_json::Map<_, _>, String>>()
                    .map(Value::Object)
            }
            b'[' => {
                let items: Vec<&RawValue> =
                    serde_json::from_str(text).map_err(|e| e.to_string())?;
                items
                    .into_iter()
                    .map(|v| value(v, depth + 1))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::Array)
            }
            _ => serde_json::from_str(text).map_err(|e| e.to_string()),
        }
    }
    raw.map_or(Ok(Value::Null), |raw| value(raw, 0))
}
pub(super) struct Bit(pub bool);
impl<'de> Deserialize<'de> for Bit {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct BitValue;
        impl de::Visitor<'_> for BitValue {
            type Value = Bit;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a Boolean or integer zero/one bit")
            }
            fn visit_bool<E: de::Error>(self, b: bool) -> Result<Bit, E> {
                Ok(Bit(b))
            }
            fn visit_u64<E: de::Error>(self, n: u64) -> Result<Bit, E> {
                match n {
                    0 => Ok(Bit(false)),
                    1 => Ok(Bit(true)),
                    _ => Err(E::custom("output contains a non-bit integer")),
                }
            }
        }
        d.deserialize_any(BitValue)
    }
}
// Missing fields are allowed individually; present null values are not bits or
// error strings. Derive rejects duplicate fields before they can be collapsed.
fn present<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(d).map(Some)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Response {
    pub id: u64,
    #[serde(default, deserialize_with = "present")]
    pub outputs: Option<Vec<Bit>>,
    #[serde(default, deserialize_with = "present")]
    pub error: Option<String>,
}
