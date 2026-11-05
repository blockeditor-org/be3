use super::*;

#[test]
fn a_documents_parts_cross_the_type_protocol_in_their_shape() {
    let body: String = (0..400_000u32)
        .map(|index| char::from(b'a' + (index.wrapping_mul(2_654_435_761) >> 27) as u8 % 26))
        .collect();
    let content = ProseContent::new(&Prose {
        body: be_block_model::Text::from(body.as_str()),
    });
    let parts = content.parts();
    let be_block_store::Part::Node { parts: children, .. } = &parts else {
        panic!("a large document is saved as a node of parts");
    };
    assert!(children.len() > 5, "the body splits into parts");

    let (bytes, shape) = be_sdk_type_api::written_shape(&parts).expect("nothing is reused");
    assert_eq!(bytes, content.encode());
    assert_eq!(be_sdk_type_api::written_parts(&bytes, &shape), parts);

    assert_eq!(
        be_sdk_type_api::written_parts(&bytes, &shape[1..]),
        be_block_store::Part::from(bytes.as_slice()),
        "a shape that doesn't match the bytes saves them as one part"
    );
}
