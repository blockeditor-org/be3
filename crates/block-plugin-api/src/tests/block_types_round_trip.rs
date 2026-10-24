use super::*;

#[test]
fn block_types_round_trip() {
    let message = Message::BlockTypes(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: [7; 16],
            display_name: "Folder".into(),
            color: Some([65, 145, 210]),
            icon_codepoint: "\u{e0d7}".into(),
            children: ChildOperations {
                add: true,
                delete: true,
                replace: false,
            },
            press: Press::Zoom,
        }],
        templates: vec![TemplateDescriptor {
            editor: [7; 16],
            template: "main".into(),
            block_type: [7; 16],
            name: "Folder".into(),
            icon_codepoint: "\u{e0d7}".into(),
            category: TemplateCategory::Important,
            dialog: false,
        }],
        editors: vec![EditorDescriptor {
            editor: [8; 16],
            edits: Edits::Any,
            name: "Debug view".into(),
            icon_codepoint: "\u{e093}".into(),
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
            color: None,
            icon_codepoint: String::new(),
            children: ChildOperations::default(),
            press: Press::Open,
        }],
        templates: Vec::new(),
        editors: Vec::new(),
    });
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
