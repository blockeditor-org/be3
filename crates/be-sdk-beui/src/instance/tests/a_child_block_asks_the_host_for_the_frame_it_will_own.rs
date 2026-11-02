use super::*;
use crate::{ChildBlock, ChildState, ChildTarget};
use be_sdk_api::ChildMode;
use beui::reactive::*;
use beui::styled::*;

const SLIDE: Uuid = Uuid::from_u128(1);
const SLIDE_TYPE: Uuid = Uuid::from_u128(2);

struct NestingRoot;

#[component]
impl crate::BeuiRoot for NestingRoot {
    #[component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        let node = {
            view! {
                <Frame>
                    <ChildBlock
                        editor={editor}
                        block={Some(ChildTarget::new(SLIDE, SLIDE_TYPE))}
                        mode=ChildMode::Live
                        own_frame=true
                        on_state={move |_: ChildState| {}}
                    />
                </Frame>
            }
        };
        view! {
            <beui::styled::Root>{node}</beui::styled::Root>
        }
    }
}

#[test]
fn a_child_block_asks_the_host_for_the_frame_it_will_own() {
    let mut session = session::<NestingRoot>(Uuid::new_v4());
    frame(&mut session, None, TopBar::Hidden);

    session.run(ViewId::MAIN, 0);
    session.run(ViewId::MAIN, 0);

    let children = session.placed_children(ViewId::MAIN);
    let child = children.first().expect("the app placed no child block");
    assert_eq!(child.content.block_id(), Some(SLIDE.into_bytes()));
    assert_eq!(child.mode, ChildMode::Live);
    assert!(
        child.own_frame,
        "a child block asking to own its frame should say so to the host"
    );
}
