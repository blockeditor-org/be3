use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_a_randomly_seeded_hash_map_in_beui() {
    let root = temporary_directory();
    let source = root.join("crates/beui-core/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"use std::collections::{BTreeMap, HashMap};
use stable_hash::HashSet;

pub struct Held {
    pub named: BTreeMap<u8, u8>,
    pub seen: HashSet<u8>,
    pub drawn: foldhash::HashSet<u8>,
}

pub fn made() -> HashMap<u8, u8> {
    std::collections::HashMap::<u8, u8>::new()
}
"#,
    )
    .unwrap();
    let elsewhere = root.join("crates/be-server/src");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(
        elsewhere.join("lib.rs"),
        "use std::collections::HashMap;\n\npub fn made() -> HashMap<u8, u8> {\n    HashMap::new()\n}\n",
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    for line in [1, 7, 11] {
        assert!(
            error.contains(&format!(
                "randomly seeded hash map: crates/beui-core/src/lib.rs:{line} "
            )),
            "{error}"
        );
    }
    assert_eq!(
        error.matches("randomly seeded hash map").count(),
        3,
        "{error}"
    );

    fs::remove_dir_all(root).unwrap();
}
