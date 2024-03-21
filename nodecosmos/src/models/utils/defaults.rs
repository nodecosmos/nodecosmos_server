use charybdis::types::{BigInt, Boolean};

pub fn default_to_false() -> Boolean {
    false
}

pub fn default_to_opt_0() -> Option<BigInt> {
    Some(0)
}
