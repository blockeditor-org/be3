use super::*;

#[test]
fn a_painting_with_bytes_after_its_end_is_unreadable() {
    let painting = triangle([255, 0, 0, 255]).encode().unwrap();
    let mut longer = painting.clone();
    longer.push(b'x');
    assert_eq!(
        crate::changed(&longer, &painting).unwrap(),
        "the accepted painting is unreadable: snapshot has 1 bytes after its end"
    );
    assert_eq!(
        crate::changed(&painting, &longer).unwrap(),
        "the painting made is unreadable: snapshot has 1 bytes after its end"
    );
    assert!(Snapshot::decode(&longer).is_err());
}
