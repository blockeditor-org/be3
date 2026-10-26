use super::*;

#[test]
fn block_types_round_trip() {
    let catalog = Catalog {
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
            plain_file: true,
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
            extensions: Vec::new(),
        }],
    };
    assert_eq!(
        crate::ops::decode::<Catalog>(&crate::ops::encode(&catalog)),
        Some(catalog)
    );
}
