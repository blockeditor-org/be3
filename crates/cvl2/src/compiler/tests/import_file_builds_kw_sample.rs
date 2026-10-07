use super::*;

#[test]
fn import_file_builds_kw_sample() {
    let artifact = import_file("kw.qxc", include_str!("../../../samples/kw.qxc"))
        .unwrap_or_else(|errors| panic!("kw.qxc failed to build: {errors:?}"));

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
            ("zero.txt", "zero".to_string()),
            ("small.txt", "small".to_string()),
            ("big.txt", "big".to_string()),
            ("negative.txt", "negative".to_string()),
        ]
    );
}
