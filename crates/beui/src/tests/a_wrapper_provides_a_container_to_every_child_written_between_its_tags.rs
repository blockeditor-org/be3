use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, narrower_than};
use crate::primitives::List;

const PAGE_WIDTH: f32 = 200.0;
const BREAKPOINT: f32 = 300.0;

#[component]
fn Page(#[prop(children)] sections: Render<(), Children<ListChild>>) -> NodeId {
    view! {
        <Container>|_size|
            <List spacing=0.0 children={sections.call(())} />
        </Container>
    }
}

#[component]
fn Section(narrows: Signal<Vec<Memo<bool>>>) -> NodeId {
    let narrow = narrower_than(BREAKPOINT);
    narrows.update(|narrows| narrows.push(narrow));
    view! {
        <Spacer />
    }
}

#[test]
fn a_wrapper_provides_a_container_to_every_child_written_between_its_tags() {
    let _root = crate::reactive::RootScope::new();
    let narrows = create_signal::<Vec<Memo<bool>>>(Vec::new());
    let document = build_node(move || {
        view! {
            <Frame width=PAGE_WIDTH>
                <Page>
                    <Section narrows />
                    <Section narrows />
                </Page>
            </Frame>
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    let narrows = narrows.get_untracked();
    assert_eq!(narrows.len(), 2);
    assert!(narrows.iter().all(|narrow| narrow.get()));
}
