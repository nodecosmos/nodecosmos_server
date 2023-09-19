pub(crate) mod base64_serializer {
    use base64::engine::general_purpose;
    use base64::Engine;
    use serde::{Deserialize, Serialize};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        let base64 = match v {
            Some(v) => {
                let b64 = general_purpose::STANDARD.encode(v);
                Some(b64)
            }
            None => None,
        };
        <Option<String>>::serialize(&base64, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let base64 = <Option<String>>::deserialize(d)?;
        match base64 {
            Some(v) => {
                let bytes = general_purpose::STANDARD.decode(v.as_bytes());

                match bytes {
                    Ok(bytes) => Ok(Some(bytes)),
                    Err(_) => Err(serde::de::Error::custom("invalid base64")),
                }
            }
            None => Ok(None),
        }
    }
}
