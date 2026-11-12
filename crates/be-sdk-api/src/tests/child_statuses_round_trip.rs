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
                named: true,
                placed: false,
            }))),
            card: true,
            title: None,
            icon: None,
            group: None,
            raised: 0,
            rung: 0,
            minimized: 0,
            fullscreen: false,
            notice: None,
            menu_asked: None,
        },
        ChildStatus {
            instance: EditorInstanceId(4),
            view: ViewId::MAIN,
            child: ChildId(2),
            available: false,
            intrinsic: None,
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
            title: Some("Terminal — ~/be3".into()),
            icon: Some(Picture {
                width: 1,
                height: 1,
                rgba: vec![1, 2, 3, 4],
            }),
            group: Some("org.example.Terminal".into()),
            raised: 3,
            rung: 1,
            minimized: 1,
            fullscreen: true,
            notice: Some("Not responding".into()),
            menu_asked: Some(MenuAsk {
                asked: 2,
                at: Pos2::new(12.0, 4.0),
            }),
        },
    ];
    assert_eq!(
        crate::ops::decode::<Vec<ChildStatus>>(&crate::ops::encode(&statuses)),
        Some(statuses),
        "statuses keep their menus, flattened, what a picker answered and what a window says"
    );
}
