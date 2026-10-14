use super::*;

const SOURCE: &str = "#builtin.build .= () => std.Folder: [\n  \"datapack\" .= std.mc.Datapack.compile: [\n    \"qxc:main\" .= main\n    \"qxc:helper\" .= helper\n  ]\n]\nmain :: () => std.mc.Result: {\n  _ = helper: {}\n  _ = twice: {}\n  _ = twice: {}\n  -> 1\n}\nhelper :: () => std.mc.Result: std.mc.runCommand: \"say helper\"\ntwice :: () => std.mc.Result: {\n  _ = std.mc.runCommand: \"say a\"\n  -> std.mc.runCommand: \"say b\"\n}\nlost :: () => std.mc.Result: {\n  r := std.mc.runCommand: \"say a\"\n  _ = std.mc.runCommand: \"say b\"\n  -> r\n}\nstd :: #builtin.std\n";

#[test]
fn import_file_datapack_names_calls_and_reports_lost_results() {
    let artifact = import_file("mc.qxc", SOURCE)
        .unwrap_or_else(|errors| panic!("mc.qxc failed to build: {errors:?}"));
    let ComptimeValueBuildArtifact::Folder(root) = artifact else {
        panic!("expected a folder");
    };
    let [(_, ComptimeValueBuildArtifact::Folder(datapack))] = root.value.as_slice() else {
        panic!("expected the datapack folder");
    };
    let files: Vec<(&str, String)> = datapack
        .value
        .iter()
        .map(|(name, artifact)| {
            let ComptimeValueBuildArtifact::File(file) = artifact else {
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
        vec![
            (
                "data/qxc/functions/main.mcfunction",
                "function qxc:helper\nfunction _0:_0\nfunction _0:_0\nreturn 1".to_string()
            ),
            (
                "data/qxc/functions/helper.mcfunction",
                "return run say helper".to_string()
            ),
            (
                "data/_0/functions/_0.mcfunction",
                "say a\nreturn run say b".to_string()
            ),
        ]
    );

    let errors = import_file(
        "mc.qxc",
        &SOURCE.replace("\"qxc:helper\" .= helper", "\"qxc:lost\" .= lost"),
    )
    .expect_err("lost's result is overwritten");
    assert_eq!(errors.len(), 1, "{errors:?}");
    let entries = &errors[0].entries;
    assert_eq!(entries[0].message, "this result was lost");
    let pos = entries[0].pos.as_ref().unwrap();
    assert_eq!((pos.fyl.as_str(), pos.lyn), ("mc.qxc", 19));
    assert_eq!(entries[1].message, "reported here");
}
