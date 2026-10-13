use super::*;

#[test]
fn an_exec_line_drops_file_codes_and_expands_the_rest() {
    let launched = entry(
        "gimp-2.10 %U --title=%c %i --from %k 100%% %f %F %u",
        "Icon=gimp",
    );
    assert_eq!(
        arguments(&launched).expect("the line expands"),
        [
            "gimp-2.10",
            "--title=Test",
            "--icon",
            "gimp",
            "--from",
            "/usr/share/applications/test.desktop",
            "100%",
        ]
    );
    let iconless = entry("app %i %d %D %n %N %v %m", "");
    assert_eq!(arguments(&iconless).expect("the line expands"), ["app"]);
    assert_eq!(arguments(&entry("%U", "")), Err(ExecError::Empty));
}
