use super::*;

fn piece(local: beui::Rect) -> Piece {
    Piece {
        local,
        source: local,
    }
}

#[test]
fn a_tooltip_over_a_view_uncovers_only_its_own_area() {
    let whole = rect(0.0, 0.0, 1.0, 1.0);
    let tooltip = rect(0.4, 0.4, 0.6, 0.5);
    let before = [piece(whole)];
    let after: Vec<Piece> = subtract(whole, &[tooltip]).into_iter().map(piece).collect();
    let changed = uncovered(&before, &after).expect("the pieces map alike");
    assert!(disjoint(&changed));
    assert!(
        (area(&changed) - tooltip.width() * tooltip.height()).abs() < 1e-6,
        "only the tooltip's area changed, not the whole region: {changed:?}"
    );
    assert!(uncovered(&after, &after).expect("unchanged").is_empty());

    let moved = [Piece {
        local: whole,
        source: rect(0.0, 0.0, 0.5, 0.5),
    }];
    assert!(
        uncovered(&before, &moved).is_none(),
        "pieces showing the texture differently are not compared"
    );
}
