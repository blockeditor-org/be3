use beui::pos2;
use beui::styled::Root;

use super::*;

struct PickRoot;

#[component]
impl BeuiRoot for PickRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let target = Some(ChildTarget::new(SLIDE, SLIDE_TYPE));
        view! {
            <Root>
                <Frame width=Size::Popover height={Size::Custom(180.0)}>
                    <ChildBlock
                        editor={editor}
                        block={target}
                        mode=ChildMode::Live
                        own_frame=true
                    />
                </Frame>
            </Root>
        }
    }
}

#[test]
fn a_pick_from_a_childs_menu_points_at_the_child_in_the_host() {
    let origin = pos2(100.0, 40.0);
    let mut test = BeuiTest::<PickRoot>::editing(Uuid::new_v4()).at_host_origin(origin);
    let placement = test.children()[0].clone();
    let anchor = placement
        .anchor
        .expect("a child placed in a frame has an anchor");

    test.host()
        .pick_child_menu(placement.child, "rename".to_owned());
    test.run();

    let picks = test.take_child_menu_picks();
    assert_eq!(picks.len(), 1);
    assert_eq!(
        picks[0].anchor,
        Some(anchor.translate(origin.to_vec2())),
        "the pick carries the child's rect in host coordinates, worked out when it was made"
    );
}
