use super::*;

#[test]
fn a_line_is_cut_to_the_terminal_without_counting_its_colors() {
    // unsafe(env)
    unsafe { std::env::set_var("COLUMNS", "6") };
    assert_eq!(fit("\x1b[1mabcdefgh\x1b[0m"), "\x1b[1mabcde\x1b[0m");
    assert_eq!(visible_width("\x1b[36;1m?\x1b[0m ab"), 4);
}
