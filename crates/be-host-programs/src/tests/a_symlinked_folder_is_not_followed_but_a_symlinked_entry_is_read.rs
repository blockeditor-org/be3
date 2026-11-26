use super::*;

#[test]
fn a_symlinked_folder_is_not_followed_but_a_symlinked_entry_is_read() {
    let scratch = Scratch::new();
    let shared = scratch.write(
        "shared/editor.desktop",
        application("Editor", "editor", "").as_bytes(),
    );
    let applications = scratch.path("data/applications");
    std::fs::create_dir_all(&applications).expect("the folder is made");
    std::os::unix::fs::symlink(&shared, applications.join("editor.desktop"))
        .expect("the entry is linked");
    std::os::unix::fs::symlink(&applications, applications.join("loop"))
        .expect("the loop is linked");
    let environment = environment(&scratch, &["data"]);
    let programs = environment.programs_among(&environment.entries());
    assert_eq!(
        ids(&programs),
        ["editor.desktop"],
        "a folder linking back to itself ends the scan rather than recursing"
    );
}
