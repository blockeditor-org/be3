use super::*;

fn solid(size: u32, color: [u8; 4]) -> Snapshot {
    Snapshot::of(
        beui_paint_snapshot::Frame {
            size: [size, size],
            pixels_per_point: 1.0,
            background: color,
            primitives: Vec::new(),
        },
        Default::default(),
    )
}

#[test]
fn the_largest_changes_come_first() {
    let root = std::env::temp_dir().join(format!("be3-review-order-{}", std::process::id()));
    let (before, after, output) = (root.join("before"), root.join("after"), root.join("output"));
    for directory in [&before, &after] {
        fs::create_dir_all(directory).unwrap();
    }
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    fs::write(
        before.join("a_small.paint"),
        solid(2, red).encode().unwrap(),
    )
    .unwrap();
    fs::write(
        after.join("a_small.paint"),
        solid(2, blue).encode().unwrap(),
    )
    .unwrap();
    fs::write(after.join("b_added.paint"), solid(3, red).encode().unwrap()).unwrap();
    fs::write(
        before.join("c_large.paint"),
        solid(400, red).encode().unwrap(),
    )
    .unwrap();
    fs::write(
        after.join("c_large.paint"),
        solid(400, blue).encode().unwrap(),
    )
    .unwrap();

    let comment = generate(&before, &after, &output, None, None).unwrap();

    let at = |name: &str| comment.find(&format!("`{name}`")).unwrap();
    assert!(at("c_large") < at("b_added"));
    assert!(at("b_added") < at("a_small"));
    assert!(comment.contains("`c_large` changed (160000 pixels)"));
    let json = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .unwrap();
    let manifest: Manifest = serde_json::from_slice(&fs::read(json).unwrap()).unwrap();
    let names: Vec<&str> = manifest.paintings.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["c_large", "b_added", "a_small"]);
    let large = manifest.paintings[0].frames[0].after.as_ref().unwrap();
    assert_ne!(large.thumbnail, large.image);
    let thumbnail = image::open(output.join(format!("{}.webp", large.thumbnail))).unwrap();
    assert_eq!((thumbnail.width(), thumbnail.height()), (240, 240));
    fs::remove_dir_all(root).unwrap();
}
