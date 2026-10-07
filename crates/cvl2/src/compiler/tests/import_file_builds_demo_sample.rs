use super::*;

fn flatten(prefix: &str, artifact: &ComptimeValueBuildArtifact, out: &mut Vec<(String, String)>) {
    match artifact {
        ComptimeValueBuildArtifact::File(file) => out.push((
            prefix.to_string(),
            String::from_utf8(file.value.clone()).unwrap(),
        )),
        ComptimeValueBuildArtifact::Folder(folder) => {
            for (name, child) in &folder.value {
                flatten(&format!("{prefix}/{name}"), child, out);
            }
        }
    }
}

#[test]
fn import_file_builds_demo_sample() {
    let artifact = import_file("demo.qxc", include_str!("../../../samples/demo.qxc"))
        .unwrap_or_else(|errors| panic!("demo.qxc failed to build: {errors:?}"));

    let mut files = Vec::new();
    flatten("", &artifact, &mut files);

    let expected = [
        ("/example", "abc"),
        (
            "/datapack/data/qxc/functions/main.mcfunction",
            "function _0:_0\nfunction _0:_1\nreturn 0",
        ),
        ("/datapack/data/_0/functions/_0.mcfunction", "return 5"),
        (
            "/datapack/data/_0/functions/_1.mcfunction",
            "say \"Hello\"\nreturn run say \"Goodbye\"",
        ),
    ];
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    assert_eq!(files, expected);
}
