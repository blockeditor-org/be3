use super::*;

#[test]
fn a_build_downloads_only_what_changed_and_drops_what_it_no_longer_has() {
    let directory = scratch("changed");
    let slot = Slot::PullRequest(7);
    let mut store = Store::default();
    let first = store.build(
        slot,
        "first",
        &[
            ("libblock_app_lib.so", b"library one"),
            ("assets/counter.cwasm", b"counter"),
            ("assets/checklist.cwasm", b"checklist"),
        ],
    );
    sync(&directory, slot, &first, &mut store).unwrap();
    assert_eq!(store.fetched.len(), 3);
    assert_eq!(
        builds::downloaded(&directory),
        Some(Downloaded {
            slot,
            commit: "first".to_owned()
        })
    );

    store.fetched.clear();
    let second = store.build(
        slot,
        "second",
        &[
            ("libblock_app_lib.so", b"library two"),
            ("assets/counter.cwasm", b"counter"),
        ],
    );
    let mut reported = Vec::new();
    builds::sync(&directory, slot, &second, &mut store, &mut |done, total| {
        reported.push((done, total));
    })
    .unwrap();

    assert_eq!(store.fetched, vec![slot.object_url(&second.files[0].hash)]);
    assert_eq!(reported.last(), Some(&(second.size(), second.size())));
    assert_eq!(
        std::fs::read(directory.join("libblock_app_lib.so")).unwrap(),
        b"library two"
    );
    assert_eq!(
        std::fs::read(directory.join("assets/counter.cwasm")).unwrap(),
        b"counter"
    );
    assert!(!directory.join("assets/checklist.cwasm").exists());
    assert_eq!(
        builds::downloaded(&directory).map(|downloaded| downloaded.commit),
        Some("second".to_owned())
    );

    let apk = directory.with_extension("apk");
    builds::fetch_launcher(&apk, slot, &second, &mut store).unwrap();
    assert_eq!(std::fs::read(&apk).unwrap(), b"launcher");
    let _ = std::fs::remove_file(&apk);
    let _ = std::fs::remove_dir_all(&directory);
}
