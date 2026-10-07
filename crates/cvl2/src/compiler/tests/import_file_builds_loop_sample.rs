use super::*;

#[test]
fn import_file_builds_loop_sample() {
    let source = include_str!("../../../samples/loop.qxc");
    let files = |source: &str| {
        let artifact = import_file("loop.qxc", source)
            .unwrap_or_else(|errors| panic!("loop.qxc failed to build: {errors:?}"));
        let ComptimeValueBuildArtifact::Folder(folder) = artifact else {
            panic!("expected a folder");
        };
        folder
            .value
            .iter()
            .map(|(name, file)| {
                let ComptimeValueBuildArtifact::File(file) = file else {
                    panic!("expected {name} to be a file");
                };
                (name.clone(), String::from_utf8(file.value.clone()).unwrap())
            })
            .collect::<Vec<_>>()
    };
    let yes = |name: &str| (name.to_string(), "yes".to_string());
    assert_eq!(
        files(source),
        [yes("sum.txt"), yes("fact.txt"), yes("collatz.txt")]
    );

    let wrong = source.replace("sum_to(10) == 55", "sum_to(10) == 54");
    assert_eq!(files(&wrong)[0], ("sum.txt".to_string(), "no".to_string()));
}
