use charybdis::{BigInt, Blob, Boolean};

#[allow(dead_code)]
pub fn default_to_true() -> Option<Boolean> {
    Some(true)
}

pub fn default_to_false() -> Option<Boolean> {
    Some(false)
}

pub fn default_to_false_bool() -> Boolean {
    false
}

pub fn default_to_0() -> Option<BigInt> {
    Some(0)
}

pub fn blob_default_to_none() -> Option<Blob> {
    None
}
