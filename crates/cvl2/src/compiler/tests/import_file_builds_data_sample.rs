use super::*;

#[test]
fn import_file_builds_data_sample() {
    let artifact = import_file("data.qxc", include_str!("../../../samples/data.qxc"))
        .unwrap_or_else(|errors| panic!("data.qxc failed to build: {errors:?}"));

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
            ("point.txt", "3,4 len2=25".to_string()),
            ("origin.txt", "0,0 len2=0".to_string()),
            ("circle.txt", "circle of radius 2".to_string()),
            ("dot.txt", "dot".to_string()),
            ("bool.txt", "false".to_string()),
        ]
    );
}
