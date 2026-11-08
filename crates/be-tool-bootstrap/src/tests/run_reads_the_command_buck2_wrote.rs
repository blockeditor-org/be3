use super::*;

#[test]
fn run_reads_the_command_buck2_wrote() {
    let file = std::env::temp_dir().join(format!("be3-run-command-{}", std::process::id()));
    std::fs::write(
        &file,
        r#"{"path":"/bin/tool","argv":["/bin/tool","a","b c"],"envp":{"HOME":"/home/a"},"is_fix_script":false,"print_command":false}"#,
    )
    .unwrap();
    let command = buck::run_command(&file).unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(command.get_program(), "/bin/tool");
    let arguments: Vec<_> = command.get_args().collect();
    assert_eq!(arguments, ["a", "b c"]);
    let environment: Vec<_> = command.get_envs().collect();
    assert_eq!(
        environment,
        [(std::ffi::OsStr::new("HOME"), Some(std::ffi::OsStr::new("/home/a")))]
    );
}
