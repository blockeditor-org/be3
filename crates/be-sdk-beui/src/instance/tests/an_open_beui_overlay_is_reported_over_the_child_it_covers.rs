use super::*;
use crate::{ChildBlock, ChildState, ChildTarget};
use be_sdk_api::ChildMode;
use beui::ItemSize;
use beui::reactive::*;
use beui::styled::*;

const BLOCK: Uuid = Uuid::from_u128(1);
const BLOCK_TYPE: Uuid = Uuid::from_u128(2);
const PANEL: f32 = 120.0;

struct OverlaidRoot;

#[component]
impl crate::BeuiApp for OverlaidRoot {
    #[component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        let node = {
            let child = NodeRef::new();
            view! {
                <Frame>
                    <List spacing=Space::None>
                        <ChildBlock
                            @node_ref={&child}
                            @sizing=ItemSize::Percent(100.0)
                            editor={editor}
                            block={Some(ChildTarget::new(BLOCK, BLOCK_TYPE))}
                            mode=ChildMode::Live
                            own_frame=true
                            on_state={move |_: ChildState| {}}
                        />
                        <Floating anchor={child}>
                            <Frame width={Size::Custom(PANEL)} height={Size::Custom(PANEL)} />
                        </Floating>
                    </List>
                </Frame>
            }
        };
        view! {
            <beui::styled::Root>{node}</beui::styled::Root>
        }
    }
}

#[test]
fn an_open_beui_overlay_is_reported_over_the_child_it_covers() {
    let mut session = session::<OverlaidRoot>(BLOCK_TYPE);
    frame(&mut session, None, TopBar::Hidden);

    session.run(ViewId::MAIN, 0);
    session.run(ViewId::MAIN, 0);

    let report = session
        .report(ViewId::MAIN)
        .expect("the frame region reports");
    let floating = report
        .floating
        .first()
        .expect("an open overlay must be reported so the host blits it over the children");
    assert_eq!(
        (floating.width(), floating.height()),
        (PANEL, PANEL),
        "the reported rectangle is the one the overlay was laid out at"
    );
    let occluder = session
        .occluders(ViewId::MAIN)
        .first()
        .expect("an open overlay must be withheld from the child under it");
    assert_eq!(
        (occluder.rect.width(), occluder.rect.height()),
        (PANEL, PANEL),
        "the occluder covers the same rectangle the overlay was laid out at"
    );
    assert!(
        occluder.after as usize >= session.placed_children(ViewId::MAIN).len(),
        "the overlay occludes every child placed under it"
    );
}
