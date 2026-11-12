use super::*;

#[test]
fn a_block_placed_twice_has_an_editor_in_each_place() {
    let block_type = Uuid::new_v4();
    let settings = app(
        "test.settings",
        vec![
            editor("list", Edits::Type(block_type.into_bytes())),
            editor("devices", Edits::Type(block_type.into_bytes())),
        ],
    );
    let (list, devices) = (
        Uuid::from_bytes(settings.editors[0].id),
        Uuid::from_bytes(settings.editors[1].id),
    );
    let registry = Rc::new(EditorRegistry::from_manifests(vec![settings]));
    let editors = OwnedEditors::install(registry, Uuid::new_v4());
    let (block, workspace) = (Uuid::new_v4(), Uuid::new_v4());
    let panel = (workspace, ViewId::MAIN, ChildId(1));
    let tab = (workspace, ViewId::MAIN, ChildId(2));
    let opened = |key: Uuid| {
        editors.with(|open| {
            open.get(&key).map(|editor| {
                let shows = editor.role().block().map(|block| block.id);
                (shows, editor.editor_id(), editor.instance())
            })
        })
    };

    let in_panel = editors.place(panel, block, block_type, None, None);
    assert_eq!(
        in_panel, block,
        "the first place a block is shown keeps its own id"
    );
    let in_tab = editors.place(tab, block, block_type, None, Some(devices));
    assert_ne!(
        in_tab, block,
        "a second place showing the block has a key of its own"
    );
    assert_eq!(editors.block(in_tab), block);
    assert_eq!(editors.parent_of(in_tab), Some(workspace));

    let (panel_shows, panel_editor, panel_instance) = opened(in_panel).unwrap();
    let (tab_shows, tab_editor, tab_instance) = opened(in_tab).unwrap();
    assert_eq!(
        (panel_shows, tab_shows),
        (Some(block), Some(block)),
        "both edit the one block"
    );
    assert_eq!(
        (panel_editor, tab_editor),
        (Some(list), Some(devices)),
        "asking for another editor in one place leaves the other place's editor alone"
    );
    assert_ne!(
        panel_instance, tab_instance,
        "each place runs an instance of its own, so each has its own frame at its own size"
    );

    assert_eq!(
        editors.place(tab, block, block_type, None, Some(devices)),
        in_tab,
        "placing again keeps the place's editor"
    );
    assert_eq!(opened(in_tab).unwrap().2, tab_instance);
    assert_eq!(editors.place(panel, block, block_type, None, None), block);
    assert_eq!(opened(in_panel).unwrap().2, panel_instance);

    editors.unplace(tab);
    assert!(
        opened(in_tab).is_none(),
        "the second place's editor goes with it"
    );
    assert_eq!(opened(in_panel).unwrap().2, panel_instance);

    let in_tab = editors.place(tab, block, block_type, None, Some(devices));
    let tab_instance = opened(in_tab).unwrap().2;
    editors.unplace(panel);
    assert_eq!(
        opened(block).map(|(_, editor, instance)| (editor, instance)),
        Some((Some(devices), tab_instance)),
        "when the place holding the block's own id goes, the one left takes it with its editor"
    );
    assert!(opened(in_tab).is_none());
    assert_eq!(editors.parent_of(block), Some(workspace));
    assert_eq!(
        editors.place(tab, block, block_type, None, Some(devices)),
        block,
        "and is placed under it from then on"
    );
    assert_eq!(opened(block).unwrap().2, tab_instance);

    editors.unplace(tab);
    assert_eq!(
        editors.place(panel, block, block_type, None, None),
        block,
        "a block no place holds is the next place's to hold"
    );

    drop(editors);
}
