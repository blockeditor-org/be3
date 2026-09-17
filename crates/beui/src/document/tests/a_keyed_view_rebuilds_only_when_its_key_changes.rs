use super::*;
use crate::reactive::{
    Button, Column, Keyed, NodeRef, ReadSignal, Row, Text, build, create_memo, create_signal, view,
};

#[test]
fn a_keyed_view_rebuilds_only_when_its_key_changes() {
    let (holder, retitle, reshape) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (holder, retitle, reshape) = (holder.clone(), retitle.clone(), reshape.clone());
        move || {
            let (state, set_state) = create_signal((0u32, "first".to_owned()));
            let edit = set_state.clone();
            view! {
                <Column spacing=0.0>
                    <Row spacing=0.0>
                        <Button
                            @node_ref=&retitle
                            on_click={move || edit.set((0, "second".to_owned()))}
                        >
                            <Text string="retitle" />
                        </Button>
                        <Button
                            @node_ref=&reshape
                            on_click={move || set_state.set((1, "third".to_owned()))}
                        >
                            <Text string="reshape" />
                        </Button>
                    </Row>
                    <Keyed @node_ref=&holder value=state key={|(shape, _): (u32, String)| shape}>
                        {|value: ReadSignal<(u32, String)>| {
                            let label = create_memo(move || value.get().1);
                            view! {
                                <Text string=label />
                            }
                        }}
                    </Keyed>
                </Column>
            }
        }
    });

    let (holder, retitle, reshape) = (holder.get(), retitle.get(), reshape.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let built = harness.document().children(holder);
    assert_eq!(text_of(harness.document(), built[0]), "first");

    harness.click(harness.center(retitle));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().children(holder),
        built,
        "a value change under the same key must reuse the built node"
    );
    assert_eq!(text_of(harness.document(), built[0]), "second");

    harness.click(harness.center(reshape));
    harness.frame(Vec::new());
    let rebuilt = harness.document().children(holder);
    assert_ne!(rebuilt, built, "a new key must build a new node");
    assert_eq!(text_of(harness.document(), rebuilt[0]), "third");
}
