use super::*;

#[test]
fn borrowed_fields_point_into_the_input() {
    let encoded = bytes(&Borrowed {
        name: "page",
        data: &[1, 2, 3],
    });
    let read: Borrowed = decode(&encoded).unwrap();
    assert_eq!(read.name, "page");
    assert_eq!(read.data, [1, 2, 3]);
    let input = encoded.as_ptr_range();
    assert!(input.contains(&read.name.as_ptr()));
    assert!(input.contains(&read.data.as_ptr()));
}
