use std::sync::Arc;

use beui::NodeId;
use beui::reactive::{Frame, ReadSignal, Text, component, create_memo, view};
use block_client::BlockClient;
use block_editor_plugin::{
    BeuiApp, ChildBlock, ChildBlockHandle, ChildMode, ChildState, ChildTarget, Editor, EditorHost,
};
use uuid::Uuid;

use crate::BeuiTest;

mod a_child_block_reports_its_placement_and_follows_its_status;

const SLIDE: Uuid = Uuid::from_u128(0x0001);
const SLIDE_TYPE: Uuid = Uuid::from_u128(0x0002);

struct ChildApp;

impl BeuiApp for ChildApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <Slide editor={editor} />
        }
    }
}

#[component]
fn Slide(editor: Editor) -> NodeId {
    let target = Some(ChildTarget::new(SLIDE, SLIDE_TYPE));
    view! {
        <Frame width=320.0 height=180.0>
            <ChildBlock editor={editor} block={target} mode=ChildMode::Preview>
                {move |handle: ChildBlockHandle| view! {
                    <Status state={handle.state} />
                }}
            </ChildBlock>
        </Frame>
    }
}

#[component]
fn Status(state: ReadSignal<ChildState>) -> NodeId {
    let text = create_memo(move || {
        state.with(|state| match (state.placed, state.available) {
            (false, _) => "unplaced".to_owned(),
            (true, false) => "waiting".to_owned(),
            (true, true) => "available".to_owned(),
        })
    });
    view! {
        <Text string={text} @test_id={"child.status"} />
    }
}

fn editor() -> BeuiTest<ChildApp> {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let host = EditorHost::default();
    host.set_editable(true);
    BeuiTest::new(Editor::new(host, client, Uuid::new_v4()))
}

fn status(test: &BeuiTest<ChildApp>) -> String {
    let node = test
        .document()
        .find_test_id("child.status")
        .expect("the status text is in the tree");
    test.document()
        .node_detail(node)
        .expect("the status text has content")
        .trim_matches('"')
        .to_owned()
}
