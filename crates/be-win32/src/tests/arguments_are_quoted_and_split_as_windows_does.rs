use super::*;

#[test]
fn arguments_are_quoted_and_split_as_windows_does() {
    assert_eq!(command_line::quote_argument("plain"), "plain");
    assert_eq!(command_line::quote_argument(""), "\"\"");
    assert_eq!(command_line::quote_argument("two words"), "\"two words\"");
    assert_eq!(command_line::quote_argument("say \"hi\""), "\"say \\\"hi\\\"\"");
    assert_eq!(command_line::quote_argument("C:\\dir with space\\"), "\"C:\\dir with space\\\\\"");
    assert_eq!(command_line::quote_argument("a\\b"), "a\\b");
    let arguments = ["--flag", "", "two words", "say \"hi\"", "C:\\dir with space\\", "a\\\\b"];
    assert_eq!(command_line::split_arguments(&command_line::join_arguments(&arguments)), arguments);
    assert_eq!(command_line::split_arguments("  -a   \"b c\"d  "), ["-a", "b cd"]);
}
