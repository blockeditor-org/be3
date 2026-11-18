use super::*;

#[test]
fn ci_names_the_changed_paintings_before_the_bxl_script() {
    let events = Path::new("target/events");
    let again = [
        "crates/a/snapshots/one.paint".to_owned(),
        "crates/b/snapshots/two.paint".to_owned(),
    ];
    assert_eq!(
        crate::verify::bxl_arguments("linux_x86_64", &["app-tests"], events, &again),
        [
            "bxl",
            "--keep-going",
            "--event-log",
            "target/events",
            "-c",
            "be3.again=crates/a/snapshots/one.paint,crates/b/snapshots/two.paint",
            "//buck/dev/verify.bxl:main",
            "--",
            "--host",
            "linux_x86_64",
            "--part",
            "app-tests",
        ]
    );
    assert!(
        !crate::verify::bxl_arguments("linux_x86_64", &[], events, &[])
            .iter()
            .any(|argument| argument == "-c")
    );
}
