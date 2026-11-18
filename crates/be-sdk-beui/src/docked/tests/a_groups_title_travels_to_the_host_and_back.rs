use super::*;

fn pane(key: &str, entries: Vec<DockSpecEntry>) -> DockSpecNode {
    DockSpecNode::Pane(DockSpecPane {
        key: key.to_owned(),
        entries,
        active: 0,
        vertical: false,
        icons_only: false,
        sidebar: 180.0,
        keep: false,
        area: None,
        bare: false,
        hide_empty_bar: false,
    })
}

#[test]
fn a_groups_title_travels_to_the_host_and_back() {
    let group = DockSpecEntry::Group {
        key: "notes".to_owned(),
        pinned: true,
        root: Box::new(pane("inner", vec![DockSpecEntry::Tab(TabId::new(5))])),
    };
    let declaration = DockDeclaration {
        spec: DockSpec {
            main: Some(pane("main", vec![group])),
            ..DockSpec::default()
        },
        tabs: vec![DockDeclaredTab {
            tab: TabId::new(5),
            key: "five".to_owned(),
            title: "Five".to_owned(),
            icon: None,
            picture: None,
            closable: false,
            back_closes: true,
            group: None,
        }],
        dialogs: Vec::new(),
        bars: Vec::new(),
        groups: vec![DockDeclaredGroup {
            key: "notes".to_owned(),
            title: "Notes".to_owned(),
        }],
        hosts: Vec::new(),
    };
    let path = |tab: TabId| Some(DockTabPath::own(tab.value()));
    let declared = declaration_to_api(
        &declaration,
        &path,
        &|_| None,
        &|_| beui::vec2(160.0, 32.0),
        &|kind| kind.to_owned(),
    );
    let nodes = declared.tree.nest().expect("the tree nests");
    assert_eq!(
        nodes
            .groups()
            .iter()
            .map(|group| (group.key.as_str(), group.title.as_str()))
            .collect::<Vec<_>>(),
        [("notes", "Notes")],
        "the host hears the group's title"
    );

    let back = declaration_from_api(&declared, &mut Paths::default()).expect("it reads back");
    assert_eq!(
        back.group_title("notes"),
        Some("Notes"),
        "and the title comes back as the declaration had it"
    );
}
