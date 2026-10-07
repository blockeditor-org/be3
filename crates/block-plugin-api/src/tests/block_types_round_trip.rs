use super::*;

#[test]
fn block_types_round_trip() {
    let message = Message::BlockTypes(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: [7; 16],
            display_name: "Folder".into(),
            icon_codepoint: "\u{e2c7}".into(),
            children: ChildOperations {
                add: true,
                delete: true,
                replace: false,
            },
        }],
        templates: vec![TemplateDescriptor {
            editor: [7; 16],
            template: "main".into(),
            block_type: [7; 16],
            name: "Folder".into(),
            icon_codepoint: "\u{e2c7}".into(),
            category: TemplateCategory::Important,
            dialog: false,
        }],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );

    let oversized = Message::BlockTypes(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: [7; 16],
            display_name: "x".repeat(MAX_STRING_BYTES + 1),
            icon_codepoint: String::new(),
            children: ChildOperations::default(),
        }],
        templates: Vec::new(),
    });
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
