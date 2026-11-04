use super::*;
use beui::Direction;
use beui::reactive::*;
use beui::styled::*;

struct SidebarRoot;

#[component]
impl crate::BeuiRoot for SidebarRoot {
    #[component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        let node = {
            let canvas = NodeRef::new();
            editor.content(&canvas);
            view! {
                <List direction=Direction::Horizontal spacing=Space::None>
                    <ListItem length=100.0>
                        <Frame />
                    </ListItem>
                    <ListItem grow=1.0>
                        <Frame @node_ref={&canvas} />
                    </ListItem>
                </List>
            }
        };
        view! {
            <beui::styled::Root>{node}</beui::styled::Root>
        }
    }
}

#[test]
fn a_focused_beui_childs_reported_content_is_its_own_canvas_not_the_whole_view() {
    let mut session = session::<SidebarRoot>(Uuid::new_v4());
    frame(
        &mut session,
        Some(Rect::from_min_size(
            beui::Pos2::new(300.0, 250.0),
            beui::Vec2::new(120.0, 90.0),
        )),
        TopBar::Shown,
    );

    session.run(ViewId::MAIN, 0);
    session.run(ViewId::MAIN, 0);

    let report = session
        .report(ViewId::MAIN)
        .expect("the frame region reports its content rect");
    let content = report.content.expect("the frame holds the content");

    assert!(
        content.left >= 99.0,
        "expected the reported content to start after the sidebar, got {:?}",
        content
    );
}
