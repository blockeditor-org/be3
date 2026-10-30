use crate::{HashMap, HashSet};

#[test]
fn same_order() {
    let keys: Vec<u64> = (0..1000).map(|key| key * 7919 % 4093).collect();
    let map = |keys: &[u64]| -> Vec<u64> {
        let mut map = HashMap::default();
        for key in keys {
            map.insert(*key, ());
        }
        for key in keys.iter().step_by(3) {
            map.remove(key);
        }
        map.into_keys().collect()
    };
    let set = |keys: &[u64]| -> Vec<u64> {
        keys.iter()
            .copied()
            .collect::<HashSet<u64>>()
            .into_iter()
            .collect()
    };
    assert_eq!(map(&keys), map(&keys));
    assert_eq!(set(&keys), set(&keys));
    let random = |keys: &[u64]| -> Vec<u64> {
        keys.iter()
            .copied()
            .collect::<foldhash::HashSet<u64>>()
            .into_iter()
            .collect()
    };
    assert_ne!(random(&keys), random(&keys));
}
