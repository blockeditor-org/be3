use super::*;

#[test]
fn only_the_files_not_already_here_are_downloaded() {
    let cache = scratch("cache");
    let mut store = Store::default();
    let app = store.put(b"the app");
    let data = store.put(b"some data");
    let first = manifest(vec![
        (
            "block-app",
            helper::File {
                executable: true,
                ..app.clone()
            },
        ),
        ("data/index.json", data.clone()),
    ]);

    let mut reported = Vec::new();
    builds::fetch(&cache, &first, &mut store, &mut |done, total| {
        reported.push((done, total));
    })
    .unwrap();
    assert_eq!(
        store.urls(),
        vec![
            helper::file_url(&app.sha256),
            helper::file_url(&data.sha256)
        ]
    );
    let size = app.size + data.size;
    assert_eq!(reported.last(), Some(&(size, size)));

    let changed = store.put(b"the app, changed");
    let second = manifest(vec![
        ("block-app", changed.clone()),
        ("data/index.json", data),
    ]);
    store.asked.clear();
    assert_eq!(builds::missing(&cache, &second), changed.size);
    builds::fetch(&cache, &second, &mut store, &mut |_, _| {}).unwrap();
    assert_eq!(store.urls(), vec![helper::file_url(&changed.sha256)]);

    let app_directory = cache.join("apps").join("android");
    builds::assemble(&cache, &first, &app_directory).unwrap();
    assert_eq!(
        std::fs::read(app_directory.join("block-app")).unwrap(),
        b"the app"
    );
    assert_eq!(
        std::fs::read(app_directory.join("data/index.json")).unwrap(),
        b"some data"
    );

    builds::forget_unused(&cache, &[&second]);
    assert!(!builds::cached(&cache, &app.sha256).exists());
    assert!(builds::cached(&cache, &changed.sha256).exists());

    let record = cache.join("installed.json");
    let installed = Installed {
        slot: Slot::PullRequest(7),
        commit: "c".repeat(40),
        product: "android".to_owned(),
    };
    builds::remember(&record, Some(&installed)).unwrap();
    assert_eq!(builds::installed(&record), Some(installed));
    builds::remember(&record, None).unwrap();
    assert_eq!(builds::installed(&record), None);
    let _ = std::fs::remove_dir_all(&cache);
}
