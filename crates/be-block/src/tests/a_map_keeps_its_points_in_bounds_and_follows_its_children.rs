use super::*;
use crate::map::{Map, MapColor, MapContent, MapCoordinate, MapPoint, MapRegion};
use crate::{BlockRef, ChildChange, Root};
use uuid::Uuid;

#[test]
fn a_map_keeps_its_points_in_bounds_and_follows_its_children() {
    let (cafe, park, copy) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let far = MapPoint::new(BlockRef::Direct(cafe), MapCoordinate::new(500.0, f64::NAN));
    let map = edited(
        &MapContent::default(),
        [
            Map::add(&far),
            Map::set_preview_region(Some(MapRegion::new(10.0, 10.0, 20.0, 20.0))),
        ],
    );
    let placed = map.root().point(far.id).unwrap();
    assert_eq!(placed.position, MapCoordinate::new(180.0, 0.0));

    let tinted = MapPoint {
        color: MapColor::Rgb {
            red: 1,
            green: 2,
            blue: 3,
        },
        ..placed
    };
    let ours = edited(&map, [Map::update(&[tinted])]);
    let moved = MapPoint {
        position: MapCoordinate::new(1.0, 2.0),
        ..placed
    };
    let theirs = edited(&map, [Map::update(&[moved])]);
    let merged = match Merge::merge3(&map, &ours, &theirs) {
        be_commit::MergeResult::Clean(merged) => merged,
        other => panic!("expected a clean merge, got {other:?}"),
    };
    let both = merged.root().point(far.id).unwrap();
    assert_eq!(both.color, tinted.color);
    assert_eq!(both.position, moved.position);

    let root = merged.root();
    let added = root.child_edit(ChildChange::Add(park)).unwrap();
    let merged = edited(&merged, [added]);
    let root = merged.root();
    let parked = root
        .points()
        .into_iter()
        .find(|point| point.block_id == BlockRef::Direct(park))
        .unwrap();
    assert_eq!(parked.position, MapCoordinate::new(15.0, 15.0));
    assert_eq!(BlockContent::references(&merged), [cafe, park]);

    let merged = edited(
        &merged,
        [
            root.child_edit(ChildChange::Delete(park)).unwrap(),
            root.child_edit(ChildChange::Replace { old: cafe, new: copy })
                .unwrap(),
        ],
    );
    assert_eq!(BlockContent::references(&merged), [copy]);
}
