use super::*;
use crate::drag::Carry;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_lifted_item_floats_as_itself_or_as_its_preview() {
    let _root = RootScope::new();
    let previewed = create_signal(false);
    let document = build_node(move || {
        crate::unstyled::drag_board();
        view! {
            <List spacing=10.0>
                <Interactive
                    drag={Func::new(move |_: PointerPress| {
                        let carry = Carry::new(1_u32);
                        Some(match previewed.get_untracked() {
                            true => carry.preview(|| view! {
                                <Frame @test_id="preview" width=30.0 height=30.0 />
                            }),
                            false => carry,
                        })
                    })}
                >
                    <Frame @test_id="item" width=100.0 height=40.0 />
                </Interactive>
                <Frame @test_id="sibling" width=100.0 height=40.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let home = harness.rect(harness.find("item"));
    let grab = home.center();
    let carry_to = |harness: &mut Harness, by: Vec2| {
        harness.down(grab, PointerButton::Primary);
        for step in 1..=4 {
            harness.frame(vec![Input::PointerMoved(grab + by * (step as f32 / 4.0))]);
        }
        harness.settle();
    };

    carry_to(&mut harness, vec2(30.0, 60.0));
    let floating = harness.rect(harness.find("item"));
    assert_eq!(
        floating.min,
        home.min + vec2(30.0, 60.0),
        "with no preview, the item itself is drawn where it floats, held where it was pressed"
    );
    harness.up(grab + vec2(30.0, 60.0), PointerButton::Primary);
    harness.settle();
    assert_eq!(harness.rect(harness.find("item")), home, "put down, it is drawn home again");

    previewed.set(true);
    carry_to(&mut harness, vec2(30.0, 60.0));
    assert_eq!(
        harness.rect(harness.find("item")),
        home,
        "with a preview, the item stays home"
    );
    let preview = harness.rect(harness.find("preview"));
    assert_eq!(preview.min, home.min + vec2(30.0, 60.0), "and the preview floats instead");
    harness.up(grab + vec2(30.0, 60.0), PointerButton::Primary);
    harness.settle();
    assert!(
        harness.document().find_test_id("preview").is_none(),
        "until it is put down"
    );
}
