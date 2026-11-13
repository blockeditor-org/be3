use super::*;

#[test]
fn a_bar_declared_on_a_pane_the_dock_cannot_name_still_reaches_the_root() {
    let declaration = DockDeclaration {
        spec: DockSpec::default(),
        tabs: Vec::new(),
        dialogs: Vec::new(),
        bars: vec![DockDeclaredBar {
            tab: TabId::new(5),
            key: "tools".to_owned(),
            kind: "tools".to_owned(),
            owner: None,
            placement: DockBarPlacement::Over(DockEdge::Top, DockAlign::Center),
            pane: Some(TabId::new(9)),
        }],
        groups: Vec::new(),
        hosts: Vec::new(),
    };
    let path = |tab: TabId| (tab == TabId::new(5)).then(|| DockTabPath::own(5));
    let declared = declaration_to_api(
        &declaration,
        &path,
        &|_| None,
        &|_| api::Size {
            width: 160.0,
            height: 32.0,
        },
        &|kind| kind.to_owned(),
    );
    assert_eq!(
        declared
            .bars
            .iter()
            .map(|bar| (bar.tab.clone(), bar.pane.clone()))
            .collect::<Vec<_>>(),
        [(DockTabPath::own(5), None)],
        "the bar is declared, with no pane, so it starts on the first"
    );
}
