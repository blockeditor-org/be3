use super::*;

#[test]
fn a_build_is_refused_when_a_file_does_not_match_its_hash() {
    let directory = scratch("mismatch");
    let slot = Slot::Main;
    let mut store = Store::default();
    let mut build = store.build(slot, "main", &[("libblock_app_lib.so", b"library")]);
    let other = store.put(slot, b"something else");
    let tampered = store.objects[&slot.object_url(&other)].clone();
    store
        .objects
        .insert(slot.object_url(&build.files[0].hash), tampered);

    let error = sync(&directory, slot, &build, &mut store).unwrap_err();

    assert!(error.contains(&other), "{error}");
    assert!(!directory.join("libblock_app_lib.so").exists());
    assert_eq!(builds::downloaded(&directory), None);

    build.files[0].hash = other;
    sync(&directory, slot, &build, &mut store).unwrap();
    assert_eq!(
        std::fs::read(directory.join("libblock_app_lib.so")).unwrap(),
        b"something else"
    );
    let _ = std::fs::remove_dir_all(&directory);
}
