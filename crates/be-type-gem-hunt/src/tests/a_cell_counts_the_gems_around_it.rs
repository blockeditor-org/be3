use super::*;

#[test]
fn a_cell_counts_the_gems_around_it() {
    // Gems at the top left corner and right of it, and one in the middle of the second row.
    let hunt = Hunt {
        gems: vec![0, 1, 9],
        ..Hunt::default()
    };
    assert_eq!(hunt.near(6), 2, "below the corner: the corner and its neighbour");
    assert_eq!(hunt.near(2), 2, "beside the second gem: it and the one below");
    assert_eq!(hunt.near(35), 0, "the far corner has none around it");
    assert_eq!(hunt.near(0), 1, "a gem does not count itself");
}
