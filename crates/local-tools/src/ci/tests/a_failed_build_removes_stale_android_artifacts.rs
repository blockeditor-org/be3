use std::fs;

#[test]
fn a_failed_build_removes_stale_android_artifacts() {
    let directory =
        std::env::temp_dir().join(format!("failed-android-build-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    for apk in ["block-app.apk", "be-launcher.apk"] {
        fs::write(directory.join(apk), b"previous signed build").unwrap();
    }
    let mut run = super::super::Run {
        options: super::super::parse(["--build-failed".into()].into_iter()).unwrap(),
        built: Vec::new(),
        failed: true,
    };
    run.android(&directory);
    assert!(run.failed);
    for apk in ["block-app.apk", "be-launcher.apk"] {
        assert!(!directory.join(apk).exists());
    }
    fs::remove_dir(directory).unwrap();
}
