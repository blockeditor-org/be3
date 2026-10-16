use super::*;

#[test]
fn remaps_that_do_not_read_stay_off_until_they_change() {
    let mut remapper = remapper("[[group]]\nrules = [{ on = \"Q\", send = \"Spcae\" }]");

    assert_eq!(remapper.input(press("Q"), NOW), vec![press("Q")]);
    assert_eq!(
        remapper.take_problems(),
        vec![
            "The input remaps could not be read, so they stay off until they change: Group 1, rule 1: no key or button is named \"Spcae\""
        ]
    );
    assert!(!remapper.running());
    assert_eq!(remapper.input(release("Q"), NOW), vec![release("Q")]);
    assert!(remapper.take_problems().is_empty(), "it says so once");

    remapper.configure(Some(
        b"[[group]]\nrules = [{ on = \"Q\", send = \"E\" }]".to_vec(),
    ));
    assert_eq!(remapper.input(press("Q"), NOW), vec![press("E")]);
}
