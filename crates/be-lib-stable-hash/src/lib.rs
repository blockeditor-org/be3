pub use std::collections::{hash_map, hash_set};

pub type State = foldhash::fast::FixedState;

pub type HashMap<K, V> = std::collections::HashMap<K, V, State>;

pub type HashSet<T> = std::collections::HashSet<T, State>;

#[cfg(test)]
mod tests;
