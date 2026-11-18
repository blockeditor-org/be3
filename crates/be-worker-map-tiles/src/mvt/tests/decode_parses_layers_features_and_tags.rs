use super::{
    GeometryKind, decode, feature, field, length_delimited, tile_with_layer, varint, zigzag,
};

#[test]
fn decode_parses_layers_features_and_tags() {
    let mut layer = length_delimited(1, b"transportation");
    layer.extend(field(5, 0));
    layer.extend(varint(8192));
    layer.extend(length_delimited(3, b"class"));
    layer.extend(length_delimited(4, &length_delimited(1, b"primary")));

    let geometry = [
        (1 << 3) | 1,
        zigzag(2),
        zigzag(2),
        (1 << 3) | 2,
        zigzag(2),
        zigzag(-1),
    ];
    layer.extend(feature(2, &[0, 0], &geometry));
    let tile = decode(&tile_with_layer(&layer)).unwrap();

    assert_eq!(tile.layers.len(), 1);
    let layer = tile.layer("transportation").unwrap();
    assert_eq!(layer.extent, 8192);
    assert_eq!(layer.keys, ["class"]);
    assert_eq!(layer.features.len(), 1);
    let street = &layer.features[0];
    assert_eq!(street.kind, GeometryKind::Line);
    assert_eq!(street.paths, [[[2.0, 2.0], [4.0, 1.0]]]);
    assert_eq!(
        layer.tag(street, "class").and_then(|value| value.as_str()),
        Some("primary")
    );
    assert!(layer.tag(street, "name").is_none());
}
