use super::*;

#[test]
fn a_held_modifier_chooses_among_its_own_places() {
    let root = RootScope::new();
    let node = origin(&root);
    let spec = closest([slot(node, 1, 0.0)])
        .when_held(Modifiers::ALT, closest([slot(node, 2, 300.0)]));

    let plain = frame(&spec, at(pos2(40.0, 10.0)), &DragFrame::default());
    assert_eq!(plain.drop, Some(1), "with nothing held it takes its own places");
    let held = frame(
        &spec,
        DragPoint {
            modifiers: Modifiers::ALT,
            ..at(pos2(40.0, 10.0))
        },
        &plain.frame,
    );
    assert_eq!(held.drop, Some(2), "with Alt held it takes the held side's");
    let again = frame(&spec, at(pos2(40.0, 10.0)), &held.frame);
    assert_eq!(
        again.frame.chosen, plain.frame.chosen,
        "and each place keeps its id whichever side chose it"
    );

    // Beside another site's place right under the item, the held side still takes it.
    let sites = closest([
        DragSpec::from(slot(node, 3, 30.0)),
        floating().when_held(Modifiers::ALT, closest([slot(node, 4, 600.0)])),
    ]);
    let near = frame(&sites, at(pos2(40.0, 10.0)), &DragFrame::default());
    assert_eq!(near.drop, Some(3));
    let forced = frame(
        &sites,
        DragPoint {
            modifiers: Modifiers::ALT,
            ..at(pos2(40.0, 10.0))
        },
        &near.frame,
    );
    assert_eq!(
        forced.drop,
        Some(4),
        "with Alt held its places are the only ones, however near another site's is"
    );
}
