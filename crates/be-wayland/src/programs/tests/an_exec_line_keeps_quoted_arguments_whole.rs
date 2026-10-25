use super::*;

#[test]
fn an_exec_line_keeps_quoted_arguments_whole() {
    let quoted = entry(
        r#""/opt/My App/run" --say "hello  world" "a \"quote\" and \$HOME and \\\\" plain"#,
        "",
    );
    assert_eq!(
        arguments(&quoted, None).expect("the line expands"),
        [
            "/opt/My App/run",
            "--say",
            "hello  world",
            r#"a "quote" and $HOME and \"#,
            "plain",
        ]
    );
    assert_eq!(
        arguments(&entry(r#"app "unterminated"#, ""), None),
        Err(ExecError::Unterminated)
    );
    assert_eq!(
        arguments(&entry(r#"sh -c "echo %f""#, ""), None).expect("the line expands"),
        ["sh", "-c", "echo "],
        "a field code inside quotes is still not a file"
    );
}
