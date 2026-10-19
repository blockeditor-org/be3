use super::*;

#[test]
fn painting_comments_render_details_tables_and_image_links() {
    let root = std::env::temp_dir().join(format!("be3-html-previews-{}", std::process::id()));
    let before = root.join("before");
    let after = root.join("after");
    let output = root.join("output");
    std::fs::create_dir_all(&before).unwrap();
    std::fs::create_dir_all(&after).unwrap();
    let snapshot = Snapshot::of(
        paint_snapshot::Frame {
            size: [2, 2],
            pixels_per_point: 1.0,
            background: [255, 0, 0, 255],
            primitives: Vec::new(),
        },
        Default::default(),
    );
    std::fs::write(after.join("sample.paint"), snapshot.encode().unwrap()).unwrap();
    let comment = generate(&before, &after, &output, None).unwrap();
    assert!(
        comment.contains("<details open=\"\"><summary><code>sample</code> was added</summary>")
    );
    assert!(
        comment.contains(
            "<table><tr><th>Frame</th><th>Before</th><th>After</th><th>Changes</th></tr>"
        )
    );
    assert!(comment.contains("<img src=\"PREVIEW_ROOT/"));
    assert!(comment.contains("alt=\"after\""));
    assert_eq!(std::fs::read_dir(&output).unwrap().count(), 1);
    std::fs::remove_dir_all(root).unwrap();
}
