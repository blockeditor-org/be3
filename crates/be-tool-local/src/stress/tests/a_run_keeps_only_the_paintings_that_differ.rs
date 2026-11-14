use super::*;

#[test]
fn a_run_keeps_only_the_paintings_that_differ() {
    let directory = env::temp_dir().join(format!("stress-paintings-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    let (made, accepted, changed) = (
        directory.join("made"),
        directory.join("accepted"),
        directory.join("changed"),
    );
    fs::create_dir_all(&made).unwrap();
    fs::create_dir_all(&accepted).unwrap();
    let painting = |text: &str| {
        beui_paint_snapshot::BrowserSnapshot::new([8, 8], text)
            .encode()
            .unwrap()
    };
    fs::write(made.join("same.browser.paint"), painting("<p>same</p>")).unwrap();
    fs::write(accepted.join("same.browser.paint"), painting("<p>same</p>")).unwrap();
    fs::write(made.join("moved.browser.paint"), painting("<p>after</p>")).unwrap();
    fs::write(
        accepted.join("moved.browser.paint"),
        painting("<p>before</p>"),
    )
    .unwrap();
    let differ = changed_paintings(&made, &accepted, &changed);
    assert_eq!(differ.len(), 1);
    assert_eq!(differ[0].0, "moved.browser.paint");
    assert_eq!(
        fs::read(changed.join("moved.browser.paint")).unwrap(),
        painting("<p>after</p>")
    );
    assert!(changed.join("moved.browser.paint.why").is_file());
    assert!(!changed.join("same.browser.paint").exists());
    let _ = fs::remove_dir_all(&directory);
}
