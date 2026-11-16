use super::*;

#[test]
fn a_lists_slots_rest_where_the_carried_row_would_land_among_rows_of_any_length() {
    let fixture = Fixture::new();
    let (_, list) = fixture.node(Rect::from_min_size(pos2(100.0, 100.0), vec2(200.0, 300.0)));
    // Rows a (20 tall), b (40, carried) and c (60), stacked from the list's top.
    let (a, _) = fixture.node(Rect::from_min_size(pos2(100.0, 100.0), vec2(200.0, 20.0)));
    let (b, _) = fixture.node(Rect::from_min_size(pos2(100.0, 120.0), vec2(200.0, 40.0)));
    let (c, _) = fixture.node(Rect::from_min_size(pos2(100.0, 160.0), vec2(200.0, 60.0)));
    let rows = ForEachRef::<char, NodeId>::new();
    let index = create_signal(0_usize).read_only();
    let fill = rows.claim();
    fill.keys(Rc::from(['a', 'b', 'c']));
    fill.rows(
        [('a', a), ('b', b), ('c', c)]
            .into_iter()
            .map(|(key, node)| ShownRow {
                key,
                index,
                nodes: vec![node],
            })
            .collect(),
    );

    let slots = slots_with(&*fixture.flat, list, rows, &'b', Direction::Vertical, None);
    let places: Vec<(Option<char>, f32)> = slots
        .iter()
        .map(|slot| (slot.drop.0, slot.rest.min.y))
        .collect();
    assert_eq!(
        places,
        vec![(None, 0.0), (Some('a'), 20.0), (Some('c'), 80.0)],
        "first: where a starts; home: where it is; after c: c's end less its own length"
    );
    assert!(
        slots.iter().all(|slot| slot.rest.height() == 40.0),
        "each rests at the carried row's own length"
    );
}
