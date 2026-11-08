use super::*;

#[test]
fn a_bundle_file_comes_from_the_first_directory_that_has_it() {
    let root = std::env::temp_dir().join(format!("be-bundle-{}", std::process::id()));
    let apps = root.join("apps");
    let libraries = root.join("libraries");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::create_dir_all(&libraries).unwrap();
    std::fs::write(apps.join("apps.json"), "[]").unwrap();
    std::fs::write(libraries.join("apps.json"), "[1]").unwrap();
    std::fs::write(libraries.join("lib.cwasm"), "").unwrap();
    let directories = [apps.clone(), libraries.clone()];

    let found = find_in(&directories, "apps.json").unwrap();
    let shared = find_in(&directories, "lib.cwasm").unwrap();
    let missing = find_in(&directories, "missing").unwrap();
    let nowhere = find_in(&[], "apps.json");
    std::fs::remove_dir_all(&root).unwrap();

    assert_eq!(found, apps.join("apps.json"));
    assert_eq!(shared, libraries.join("lib.cwasm"));
    assert_eq!(missing, apps.join("missing"));
    assert!(nowhere.is_err());
}
