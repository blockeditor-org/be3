use super::*;

#[test]
fn import_file_builds_text_sample() {
    let artifact = import_file("text.qxc", include_str!("../../../samples/text.qxc"))
        .unwrap_or_else(|errors| panic!("text.qxc failed to build: {errors:?}"));

    let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
        panic!("expected a folder");
    };
    let files: Vec<(&str, String)> = folder
        .value
        .iter()
        .map(|(name, file)| {
            let ComptimeValueBuildArtifact::File(file) = file else {
                panic!("expected {name} to be a file");
            };
            (
                name.as_str(),
                String::from_utf8(file.value.clone()).unwrap(),
            )
        })
        .collect();
    assert_eq!(
        files,
        [
            (
                "squares.txt",
                "1 squared is 1\n2 squared is 4\n3 squared is 9\n4 squared is 16".to_string()
            ),
            (
                "greeting.txt",
                "hello, Ada!\nhello, Grace!\nhello, Linus!\n".to_string()
            ),
        ]
    );
}
