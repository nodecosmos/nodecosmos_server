use std::collections::{HashMap, HashSet};

use charybdis::types::Uuid;

pub trait ToHashSet<T> {
    fn to_hash_set(self) -> HashSet<T>;
}

impl<T> ToHashSet<T> for Vec<T>
where
    T: std::hash::Hash + Eq,
{
    fn to_hash_set(self) -> HashSet<T> {
        self.into_iter().collect()
    }
}

/// Trait to convert a HashMap with Vec values to a HashMap with HashSet values
pub trait HashMapVecValToSet<T> {
    fn hash_map_vec_val_to_set(self) -> HashMap<T, HashSet<T>>;
}

impl<T: std::hash::Hash + Eq> HashMapVecValToSet<T> for HashMap<T, Vec<T>> {
    fn hash_map_vec_val_to_set(self) -> HashMap<T, HashSet<T>> {
        self.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect()
    }
}

pub trait RefCloned<T> {
    type Iterable<I>;

    fn ref_cloned(&self) -> Self::Iterable<T>;
}

impl<T: Clone> RefCloned<T> for Option<Vec<T>> {
    type Iterable<I> = Vec<T>;

    fn ref_cloned(&self) -> Vec<T> {
        self.as_ref().cloned().unwrap_or_default()
    }
}

impl<T: Clone> RefCloned<T> for Option<HashSet<T>> {
    type Iterable<I> = HashSet<T>;

    fn ref_cloned(&self) -> HashSet<T> {
        self.as_ref().cloned().unwrap_or_default()
    }
}

pub trait Merge: Sized {
    fn merge(&mut self, other: Self);

    fn merge_unique(&mut self, other: Self);
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
}

impl Merge for HashSet<Uuid> {
    fn merge(&mut self, other: Self) {
        self.extend(other);
    }

    fn merge_unique(&mut self, other: Self) {
        self.extend(other);
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
}

pub trait ChainOptRef<'a, T> {
    fn chain_opt_ref(&'a self, other: &'a Option<Vec<T>>) -> Option<Vec<&'a T>>;
}

impl<'a, T> ChainOptRef<'a, T> for Option<Vec<T>> {
    fn chain_opt_ref(&'a self, other: &'a Option<Vec<T>>) -> Option<Vec<&'a T>> {
        match (self, other) {
            (Some(vec1), Some(vec2)) => {
                let res = vec1.iter().chain(vec2.iter()).collect::<Vec<&T>>();
                Some(res)
            }
            (Some(vec1), None) => Some(vec1.iter().collect()),
            (None, Some(vec2)) => Some(vec2.iter().collect()),
            (None, None) => None,
        }
    }
}

pub trait HashMapVecToSet<T> {
    fn hash_map_vec_to_set(self) -> HashMap<T, HashSet<T>>;
}

impl<T: std::hash::Hash + Eq> HashMapVecToSet<T> for HashMap<T, Vec<T>> {
    fn hash_map_vec_to_set(self) -> HashMap<T, HashSet<T>> {
        self.into_iter().map(|(k, v)| (k, v.into_iter().collect())).collect()
    }
}
