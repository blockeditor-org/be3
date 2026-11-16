use super::*;

#[test]
fn an_item_from_another_list_gets_a_place_at_each_gap_its_size() {
    let fixture = Fixture::new();
    let (_, list) = fixture.node(Rect::from_min_size(pos2(100.0, 100.0), vec2(200.0, 300.0)));
    // Rows a (20 tall) and c (60), 10 apart; 'x' comes from another list, 30 tall.
    let (a, _) = fixture.node(Rect::from_min_size(pos2(100.0, 100.0), vec2(200.0, 20.0)));
    let (c, _) = fixture.node(Rect::from_min_size(pos2(100.0, 130.0), vec2(200.0, 60.0)));
    let rows = ForEachRef::<char, NodeId>::new();
    let index = create_signal(0_usize).read_only();
    let fill = rows.claim();
    fill.keys(Rc::from(['a', 'c']));
    fill.rows(
        [('a', a), ('c', c)]
            .into_iter()
            .map(|(key, node)| ShownRow {
                key,
                index,
                nodes: vec![node],
            })
            .collect(),
    );

    let item = Some(vec2(180.0, 30.0));
    let slots = slots_with(&*fixture.flat, list, rows, &'x', Direction::Vertical, item);
    let places: Vec<(Option<char>, f32)> = slots
        .iter()
        .map(|slot| (slot.drop.0, slot.rest.min.y))
        .collect();
    assert_eq!(
        places,
        vec![(None, 0.0), (Some('a'), 30.0), (Some('c'), 100.0)],
        "before a: where a starts; before c: where c starts; past c: c's end and the spacing"
    );
    assert!(
        slots.iter().all(|slot| slot.rest.size() == vec2(180.0, 30.0)),
        "each rests at the carried item's size"
    );

    let none = slots_with(&*fixture.flat, list, rows, &'x', Direction::Vertical, None);
    assert!(none.is_empty(), "with no item carried there is nothing to size a place by");
}
