use super::*;

#[test]
fn child_statuses_round_trip() {
    let statuses = vec![
        ChildStatus {
            instance: EditorInstanceId(4),
            region: EditorRegion::Frame,
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
            creation: Some(CreationProgress::Options { ready: true }),
            settings: Some(SettingsProgress {
                changed: true,
                summary: Some("Every 5 minutes".into()),
            }),
            preview: ChildPreview::Image(PreviewImage {
                image: 12,
                width: 320,
                height: 180,
            }),
        },
        ChildStatus {
            instance: EditorInstanceId(4),
            region: EditorRegion::Frame,
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
            settings: None,
            preview: Default::default(),
        },
    ];
    assert_eq!(
        crate::ops::decode::<Vec<ChildStatus>>(&crate::ops::encode(&statuses)),
        Some(statuses),
        "statuses keep their menus, flattened"
    );
}
