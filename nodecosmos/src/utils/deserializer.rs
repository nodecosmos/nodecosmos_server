use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;
use std::fmt;

/// Allow optional fields to be None, but require them to be provided within JSON
pub fn required<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct OptionVisitor<T>(std::marker::PhantomData<T>);

    impl<'de, T> Visitor<'de> for OptionVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Option<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("an Option")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            T::deserialize(deserializer).map(Some)
        }
    }

    deserializer
        .deserialize_option(OptionVisitor(std::marker::PhantomData))
        .or_else(|err| Err(de::Error::custom(format!("Field is missing from JSON: {}", err))))
}
