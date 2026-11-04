use crate::disk::name_problem;

#[test]
fn a_file_name_must_work_on_every_system() {
    for fine in [
        "main.rs",
        ".gitignore",
        "Cargo.lock",
        "a b",
        "COM10",
        "console.log",
        "naïve.md",
    ] {
        assert_eq!(name_problem(fine), None, "{fine}");
    }
    for bad in [
        "",
        ".",
        "..",
        "a/b",
        "a\\b",
        "a:b",
        "what?",
        "star*",
        "pipe|",
        "quote\"",
        "<a>",
        "tab\there",
        "dot.",
        "space ",
        "CON",
        "con.txt",
        "NUL",
        "Aux.md",
        "COM1",
        "lpt9.log",
    ] {
        assert!(name_problem(bad).is_some(), "{bad:?}");
    }
    assert!(name_problem(&"a".repeat(256)).is_some());
    assert_eq!(name_problem(&"a".repeat(255)), None);
}
