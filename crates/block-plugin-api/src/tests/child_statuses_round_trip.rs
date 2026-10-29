use super::*;

#[test]
fn child_statuses_round_trip() {
    let statuses = vec![
        ChildStatus {
            instance: EditorInstanceId(4),
            view: ViewId::MAIN,
            child: ChildId(1),
            available: true,
            intrinsic: Some(Size {
                width: 320.0,
                height: 180.0,
            }),
            aspect_ratio: Some(16.0 / 9.0),
            active: false,
            interaction: InteractionMode::Live,
            capabilities: EditorCapabilities::default(),
            resize: ResizeMode::Both,
            error: None,
            menu: MenuEntry::flatten(&[MenuEntry::Command(MenuCommand {
                id: "editor.rename".into(),
                label: "Rename".into(),
                glyph: "edit".into(),
                enabled: true,
                ..MenuCommand::default()
            })]),
            creation: Some(CreationProgress::Options),
            preview: ChildPreview::Image(PreviewImage {
                image: 12,
                width: 320,
                height: 180,
            }),
            dialog: DialogStatus {
                handles_dismiss: true,
                answer: Some(DialogAnswer::Accepted),
            },
            picked: Some(Ok(Some(PickedBlock {
                block_id: [7; 16],
                block_type: [8; 16],
                linked: true,
                placed: false,
            }))),
            card: true,
        },
        ChildStatus {
            instance: EditorInstanceId(4),
            view: ViewId::MAIN,
            child: ChildId(2),
            available: false,
            intrinsic: None,
            aspect_ratio: None,
            active: false,
            interaction: InteractionMode::Live,
            capabilities: EditorCapabilities::default(),
            resize: ResizeMode::Both,
            error: Some("the block is already open above this editor".into()),
            menu: Vec::new(),
            creation: None,
            preview: Default::default(),
            dialog: DialogStatus::default(),
            picked: None,
            card: false,
        },
    ];
    assert_eq!(
        crate::ops::decode::<Vec<ChildStatus>>(&crate::ops::encode(&statuses)),
        Some(statuses),
        "statuses keep their menus, flattened, and what a picker answered"
    );
}
