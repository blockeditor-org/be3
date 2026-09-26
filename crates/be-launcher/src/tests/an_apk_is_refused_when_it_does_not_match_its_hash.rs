use super::*;

#[test]
fn an_apk_is_refused_when_it_does_not_match_its_hash() {
    let directory = scratch("mismatch");
    let apk = directory.join("app.apk");
    let slot = Slot::Main;
    let mut store = Store::default();
    let mut object = store.put(slot, b"app");
    let other = store.put(slot, b"something else");
    let tampered = store.objects[&slot.object_url(&other.hash)].clone();
    store
        .objects
        .insert(slot.object_url(&object.hash), tampered);

    let error = fetch_apk(&apk, slot, &object, &mut store).unwrap_err();

    assert!(error.contains(&other.hash), "{error}");
    assert!(!apk.exists());

    object.hash = other.hash;
    fetch_apk(&apk, slot, &object, &mut store).unwrap();
    assert_eq!(std::fs::read(&apk).unwrap(), b"something else");
    let _ = std::fs::remove_dir_all(&directory);
}
