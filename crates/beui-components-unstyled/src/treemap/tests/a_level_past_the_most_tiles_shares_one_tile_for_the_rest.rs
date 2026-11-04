use super::*;

#[test]
fn a_level_past_the_most_tiles_shares_one_tile_for_the_rest() {
    let items: Vec<TreemapItem<usize>> = (0..TREEMAP_MOST_TILES * 3)
        .map(|key| TreemapItem {
            key,
            label: format!("{key}"),
            size: key as u64,
            opens: false,
        })
        .collect();
    let tiles = shown_tiles(&items, TREEMAP_MOST_TILES);
    assert_eq!(tiles.len(), TREEMAP_MOST_TILES);
    let largest = items.len() - 1;
    assert_eq!(tiles[0].key, TreemapTileKey::Item(largest));
    assert!(
        tiles
            .windows(2)
            .take(TREEMAP_MOST_TILES - 2)
            .all(|pair| pair[0].size >= pair[1].size),
        "the tiles come largest first"
    );
    let rest = tiles.last().expect("a rest tile");
    assert_eq!(rest.key, TreemapTileKey::Rest);
    let total: u64 = items.iter().map(|item| item.size).sum();
    assert_eq!(tiles.iter().map(|tile| tile.size).sum::<u64>(), total);
    assert!(
        !tiles.iter().any(|tile| tile.key == TreemapTileKey::Item(0)),
        "an item of no size takes no tile"
    );
}
