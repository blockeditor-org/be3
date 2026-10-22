use super::*;

#[test]
fn peers_formatting_and_typing_at_once_converge() {
    let mut content = note("hello world");
    let left = live(&content);
    let right = live(&content);

    left.edit(Vec::new(), &mut |edit| {
        edit.format(6..11, &change(BOLD, Some(Value::Bool(true))));
    });
    right.edit(Vec::new(), &mut |edit| edit.replace(8, 0, b"--"));
    let from_left = send(&left, &mut content);
    let from_right = send(&right, &mut content);
    for edit in &from_right {
        assert!(left.apply(edit));
    }
    for edit in &from_left {
        assert!(right.apply(edit));
    }

    assert_eq!(left.bytes(), b"hello wo--rld");
    assert_eq!(right.bytes(), left.bytes());
    for index in 0..left.bytes().len() {
        assert_eq!(
            formatted(&left, index, BOLD),
            formatted(&right, index, BOLD),
            "both peers agree at {index}"
        );
        assert_eq!(formatted(&left, index, BOLD), index >= 6, "bold at {index}");
    }
    let reread = live(&content);
    assert_eq!(reread.bytes(), left.bytes());
    assert!(formatted(&reread, 8, BOLD));
}
