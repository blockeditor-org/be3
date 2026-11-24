use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

/// Two sections of a 200-point scroll, each a 40-point sticky header over five 40-point rows.
#[test]
fn a_sticky_header_stays_at_the_top_of_its_scroll_until_its_section_ends() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let headers = [NodeRef::new(), NodeRef::new()];
    let first_row = NodeRef::new();
    let pressed = create_signal(Vec::<&'static str>::new());
    let document = build_node(move || {
        let section = move |header: NodeRef, row: Option<NodeRef>| {
            view! {
                <List spacing=0.0>
                    <ListItem sticky=true>
                        <Interactive
                            on_press={move |_: PointerPress| pressed.update(|pressed| pressed.push("header"))}
                        >
                            <Frame @node_ref=&header height=40.0 />
                        </Interactive>
                    </ListItem>
                    {row.map(|row| view! {
                        <Frame @node_ref=&row height=40.0 />
                    })}
                    <ForEach value={indices(if row.is_some() { 4 } else { 5 })}>|_: usize|
                        <Interactive
                            on_press={move |_: PointerPress| pressed.update(|pressed| pressed.push("row"))}
                        >
                            <Frame height=40.0 />
                        </Interactive>
                    </ForEach>
                </List>
            }
        };
        view! {
            <List spacing=0.0>
                <ListItem length=200.0>
                    <Scroll @node_ref=&scroll style={plain_scroll()}>
                        {section(headers[0], Some(first_row))}
                        {section(headers[1], None)}
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::sized(document, vec2(200.0, 200.0));
    harness.frame(Vec::new());
    let top = |harness: &Harness, node: NodeRef| harness.rect(node.get()).top();
    assert_eq!(top(&harness, headers[0]), 0.0);
    assert_eq!(top(&harness, headers[1]), 240.0);

    let scroll_to = |harness: &mut Harness, offset: f32| {
        let document = harness.document_mut();
        let node = document
            .first_offset_within(scroll)
            .expect("a scroll lays its content out in an offset");
        document.drive_offset(node, offset);
        harness.frame(Vec::new());
    };

    scroll_to(&mut harness, 100.0);
    assert_eq!(
        top(&harness, headers[0]),
        0.0,
        "scrolled past, a sticky header stays at the top of what is shown"
    );
    assert_eq!(
        top(&harness, first_row),
        -60.0,
        "while its section's rows scroll on under it"
    );

    scroll_to(&mut harness, 220.0);
    assert_eq!(
        top(&harness, headers[0]),
        -20.0,
        "its section's end carries it off as the next section comes up"
    );
    assert_eq!(top(&harness, headers[1]), 20.0);

    scroll_to(&mut harness, 300.0);
    assert_eq!(
        top(&harness, headers[1]),
        0.0,
        "and the next header sticks in its place"
    );

    harness.click(pos2(100.0, 20.0));
    assert_eq!(
        pressed.get_untracked(),
        vec!["header"],
        "a sticky header takes the press over the row scrolled under it"
    );
}
