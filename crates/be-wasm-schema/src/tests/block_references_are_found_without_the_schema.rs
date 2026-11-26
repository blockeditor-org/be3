use super::*;

#[test]
fn block_references_are_found_without_the_schema() {
    let linked = Linked {
        to: Reference([1; 16]),
        many: vec![Reference([2; 16]), Reference([3; 16])],
        child: Some(Box::new(Linked {
            to: Reference([4; 16]),
            many: Vec::new(),
            child: None,
        })),
    };
    let encoded = bytes(&linked);
    let found = references(&encoded).unwrap();
    assert_eq!(found, [1, 2, 3, 4].map(|byte| Reference([byte; 16])));
    // Even where no type reading it knows the fields holding them.
    let kept: KeepsAll = decode(&encoded).unwrap();
    assert_eq!(references(&bytes(&kept)).unwrap(), found);
    let text = dump(&encoded).unwrap();
    assert!(text.contains("reference 01010101"), "{text}");
    assert!(
        dump(&bytes(&person_v2()))
            .unwrap()
            .contains("\"ada@example.com\"")
    );
}
