use super::*;
use beui::{WindowAnchor, WindowHandle};

/// A window of its own that blocks its owner dims the owner in the owner's window, where the
/// lock screen would also be, and leaves the rest of that window bright.
#[test]
fn a_window_of_its_own_that_blocks_dims_its_owner_in_the_owners_window() {
    let _root = RootScope::new();
    let owner = NodeRef::new();
    let owned = create_signal(None::<NodeId>);
    let anchor = beui::Rect::from_min_size(pos2(40.0, 40.0), Vec2::new(120.0, 30.0));
    let document = build(move || {
        view! {
            <Root>
                <List spacing=Space::None>
                <List direction=beui::Direction::Horizontal spacing=Space::Lg>
                    <Frame @node_ref=&owner fill=ColorRole::Surface width=Size::Custom(360.0) height=Size::Custom(300.0)>
                        <Text>"The owner, dimmed while its window is open"</Text>
                    </Frame>
                    <Frame fill=ColorRole::Surface width=Size::Custom(360.0) height=Size::Custom(300.0)>
                        <Text>"Beside it, left bright"</Text>
                    </Frame>
                </List>
                <Window
                    size={Vec2::new(240.0, 120.0)}
                    owner={owned}
                    anchor={Some(WindowAnchor::Beside(anchor))}
                    blocks=true
                    scrim={beui::Color32::from_rgba_unmultiplied(0, 0, 0, 140)}
                >|_: WindowHandle|
                    <Text>"A dialog"</Text>
                </Window>
                </List>
            </Root>
        }
    });
    let size = Vec2::new(800.0, 360.0);
    let mut test = DocumentTest::hosting_windows(document, size);
    owned.set(Some(owner.get()));
    test.frame(Vec::new());
    test.frame(Vec::new());
    test.snapshot("a_window_of_its_own_dims_its_owner");
}
