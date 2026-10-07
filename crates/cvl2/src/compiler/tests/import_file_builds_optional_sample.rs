use super::*;

#[test]
fn import_file_builds_optional_sample() {
    let artifact = import_file(
        "optional.qxc",
        include_str!("../../../samples/optional.qxc"),
    )
    .unwrap_or_else(|errors| panic!("optional.qxc failed to build: {errors:?}"));

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
            ("found.txt", "found at 1".to_string()),
            ("missing.txt", "not found".to_string()),
            ("forced.txt", "2".to_string()),
            ("flag.txt", "set".to_string()),
        ]
    );
}
