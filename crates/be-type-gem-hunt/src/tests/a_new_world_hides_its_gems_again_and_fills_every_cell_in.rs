use super::*;

#[test]
fn a_new_world_hides_its_gems_again_and_fills_every_cell_in() {
    let start = HuntContent::new(&Hunt::scattered(7));
    let gems = start.root().gems;
    assert_eq!(gems.len(), GEMS as usize);
    assert!(gems.iter().all(|&cell| cell < CELLS));
    assert_eq!(
        Hunt::scattered(7).gems,
        gems,
        "a seed scatters the same way each time"
    );

    let player = Uuid::from_u128(1);
    let dug = edited(&start, [Hunt::dig(gems[0], player), Hunt::dig(35, player)]);
    assert_eq!(dug.root().dug.len(), 2);

    let again = edited(&dug, [dug.root().new_world(8)]);
    let hunt = again.root();
    assert!(hunt.dug.is_empty(), "every cell is filled in");
    assert_eq!(hunt.gems, Hunt::scattered(8).gems);
    assert_ne!(hunt.gems, gems, "the gems lie elsewhere");
}
