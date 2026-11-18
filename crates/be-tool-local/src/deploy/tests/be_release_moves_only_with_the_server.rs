use super::*;

fn options(flags: &[&str]) -> Result<Options, String> {
    let base = ["--repository", ".", "--server", "server"];
    parse(base.iter().chain(flags).map(|flag| flag.to_string()))
}

#[test]
fn be_release_moves_only_with_the_server() {
    assert_eq!(
        be_release(&options(&[]).unwrap(), "abc123").as_deref(),
        Some("abc123\n")
    );
    assert_eq!(
        be_release(&options(&["--skip-server"]).unwrap(), "abc123"),
        None
    );
    assert_eq!(
        be_release(&options(&["--skip-server", "--skip-be"]).unwrap(), "abc123"),
        None
    );
    let refused = options(&["--skip-be"]).err().unwrap();
    assert!(
        refused.contains("--skip-be needs --skip-server"),
        "{refused}"
    );
}
