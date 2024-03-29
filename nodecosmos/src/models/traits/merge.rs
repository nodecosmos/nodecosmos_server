use charybdis::types::Uuid;
use std::collections::{HashMap, HashSet};

pub trait Merge: Sized {
    fn merge(&mut self, other: Self);

    fn merge_unique(&mut self, other: Self);

    fn unmerge(&mut self, other: Self);
}

impl Merge for HashMap<Uuid, Vec<Uuid>> {
    fn merge(&mut self, other: Self) {
        for (key, values) in other {
            self.entry(key)
                .and_modify(|e| e.merge_unique(values.clone()))
                .or_insert_with(|| values);
        }
    }

    fn merge_unique(&mut self, other: Self) {
        self.merge(other)
    }

    fn unmerge(&mut self, other: Self) {
        for (key, values) in other {
            self.entry(key)
                .and_modify(|e| e.retain(|v| !values.contains(v)))
                .or_insert_with(|| values);
        }
    }
}

impl Merge for HashMap<Uuid, HashSet<Uuid>> {
    fn merge(&mut self, other: Self) {
        for (key, values) in other {
            self.entry(key)
                .and_modify(|e| e.merge(values.clone()))
                .or_insert_with(|| values);
        }
    }

    fn merge_unique(&mut self, other: Self) {
        for (key, values) in other {
            self.entry(key)
                .and_modify(|e| e.merge_unique(values.clone()))
                .or_insert_with(|| values);
        }
    }

    fn unmerge(&mut self, other: Self) {
        for (key, values) in other {
            self.entry(key)
                .and_modify(|e| e.retain(|v| !values.contains(v)))
                .or_insert_with(|| values);
        }
    }
}

impl Merge for Vec<Uuid> {
    fn merge(&mut self, mut other: Self) {
        self.append(&mut other);
    }

    fn merge_unique(&mut self, mut other: Vec<Uuid>) {
        let mut seen = HashSet::new();

        other.retain(|item| seen.insert(*item));
        self.retain(|item| seen.insert(*item));

        self.append(&mut other);
    }

    fn unmerge(&mut self, other: Self) {
        self.retain(|v| !other.contains(v));
    }
}

impl Merge for HashSet<Uuid> {
    fn merge(&mut self, other: Self) {
        self.extend(other);
    }

    fn merge_unique(&mut self, other: Self) {
        self.extend(other);
    }

    fn unmerge(&mut self, other: Self) {
        self.retain(|v| !other.contains(v));
    }
}

impl<T: Merge> Merge for Option<T> {
    fn merge(&mut self, other: Self) {
        if let Some(other) = other {
            if let Some(self_map) = self {
                self_map.merge(other);
            } else {
                *self = Some(other);
            }
        }
    }

    fn merge_unique(&mut self, other: Self) {
        if let Some(other) = other {
            if let Some(self_map) = self {
                self_map.merge_unique(other);
            } else {
                *self = Some(other);
            }
        }
    }

    fn unmerge(&mut self, other: Self) {
        if let Some(other) = other {
            if let Some(self_map) = self {
                self_map.unmerge(other);
            }
        }
    }
}
