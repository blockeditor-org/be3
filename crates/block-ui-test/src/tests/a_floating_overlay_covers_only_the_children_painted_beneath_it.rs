use beui::pos2;
use beui::styled::Root;

use super::*;
use beui::LayerPolicy;

const WINDOWED: Uuid = Uuid::from_u128(0x0011);
const BENEATH: Uuid = Uuid::from_u128(0x0012);

struct WindowApp;

#[component]
impl BeuiApp for WindowApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Window editor={editor} />
            </Root>
        }
    }
}

#[component]
fn Window(editor: Editor) -> NodeId {
    let windowed = Some(ChildTarget::new(WINDOWED, SLIDE_TYPE));
    let beneath = Some(ChildTarget::new(BENEATH, SLIDE_TYPE));
    view! {
        <List spacing=Space::None>
            <Overlay
                anchor={OverlayAnchor::Point(pos2(40.0, 40.0))}
                policy=LayerPolicy::Pass
                open=true
            >
                <Frame width=Size::Menu height={Size::Custom(120.0)}>
                    <ChildBlock editor={editor} block={windowed} mode=ChildMode::Live />
                </Frame>
            </Overlay>
            <Frame width=Size::Popover height={Size::Custom(240.0)}>
                <ChildBlock editor={editor} block={beneath} mode=ChildMode::Live />
            </Frame>
        </List>
    }
}

#[test]
fn a_floating_overlay_covers_only_the_children_painted_beneath_it() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<WindowApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    test.run();
    test.run();

    let placed: Vec<Option<[u8; 16]>> = test
        .children()
        .iter()
        .map(|placement| placement.content.block_id())
        .collect();
    assert_eq!(
        placed,
        [Some(BENEATH.into_bytes()), Some(WINDOWED.into_bytes())],
        "the child in the floating overlay is placed after the one it paints over"
    );
    let covering: Vec<u32> = test
        .occluders()
        .iter()
        .map(|occluder| occluder.after)
        .collect();
    assert_eq!(
        covering,
        [1],
        "the overlay covers the child beneath it and not the one inside it"
    );
}
