use super::*;

#[test]
fn a_deep_tile_is_cut_from_its_source() {
    let deep = TileId {
        zoom: 16,
        x: 43,
        y: 83,
    };
    let source = TileId {
        zoom: SOURCE_MAX_ZOOM,
        x: 10,
        y: 20,
    };

    assert_eq!(deep.source(), source);
    assert_eq!(
        window(deep, source),
        Window {
            factor: 4.0,
            offset: [3.0, 3.0],
        }
    );
}
