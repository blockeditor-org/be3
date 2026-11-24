use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_sheet_rises_as_it_opens_on_a_spring() {
    let _root = crate::reactive::RootScope::new();
    let content = NodeRef::new();
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame />
                </ListItem>
                <ModalSheet open={open} fit=true on_close={move || open.set(false)}>
                    <Frame @node_ref=&content height=120.0 />
                </ModalSheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    open.set(true);
    let mut tops = Vec::new();
    for _ in 0..120 {
        harness.frame(Vec::new());
        if let Some(rect) = content
            .try_get()
            .and_then(|node| harness.document().node_rect(node))
        {
            tops.push(rect.top());
        }
    }
    let rises: Vec<f32> = tops
        .windows(2)
        .map(|pair| pair[0] - pair[1])
        .skip_while(|rise| *rise <= 0.0)
        .collect();
    assert!(
        rises.len() > 2 && rises[0] < rises[1],
        "it sets out from still and speeds up, as a spring does (an ease-out moves most at \
         first): {tops:?}"
    );
    assert!(
        rises.iter().all(|rise| *rise >= 0.0),
        "and eases onto its stop without passing it: {tops:?}"
    );
    assert_eq!(
        tops.last().copied(),
        Some(480.0),
        "where it rests: {tops:?}"
    );
}
