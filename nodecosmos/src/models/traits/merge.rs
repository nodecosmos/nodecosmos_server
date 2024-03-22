use charybdis::types::Uuid;
use std::collections::HashMap;

pub trait Merge {
    fn merge(&mut self, other: Self);
}

impl Merge for HashMap<Uuid, Vec<Uuid>> {
    fn merge(&mut self, other: Self) {
        for (key, mut values) in other {
            self.entry(key).and_modify(|e| e.append(&mut values)).or_insert(values);
        }
    }
}

impl Merge for Option<HashMap<Uuid, Vec<Uuid>>> {
    fn merge(&mut self, other: Self) {
        if let Some(other) = other {
            if let Some(self_map) = self {
                self_map.merge(other);
            } else {
                *self = Some(other);
            }
        }
    }
}

impl Merge for Vec<Uuid> {
    fn merge(&mut self, mut other: Self) {
        self.append(&mut other);
    }
}

impl Merge for Option<Vec<Uuid>> {
    fn merge(&mut self, other: Self) {
        if let Some(other) = other {
            if let Some(self_vec) = self {
                self_vec.merge(other);
            } else {
                *self = Some(other);
            }
        }
    }
}
