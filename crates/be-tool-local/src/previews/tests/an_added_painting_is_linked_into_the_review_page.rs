use super::*;

#[test]
fn an_added_painting_is_linked_into_the_review_page() {
    let root = std::env::temp_dir().join(format!("be3-review-previews-{}", std::process::id()));
    let before = root.join("before");
    let after = root.join("after");
    let output = root.join("output");
    let site = root.join("site");
    for directory in [&before, &after, &site] {
        fs::create_dir_all(directory).unwrap();
    }
    fs::write(site.join("index.html"), "<html></html>").unwrap();
    let snapshot = Snapshot::of(
        beui_paint_snapshot::Frame {
            size: [2, 2],
            pixels_per_point: 1.0,
            background: [255, 0, 0, 255],
            primitives: Vec::new(),
        },
        Default::default(),
    );
    fs::write(after.join("sample.paint"), snapshot.encode().unwrap()).unwrap();
    let comment = generate(&before, &after, &output, None, Some(&site)).unwrap();
    assert!(
        comment.contains(
            "One painting differs in this pull request: 0 changed, 1 added, 0 removed.\n"
        )
    );
    let files: Vec<String> = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    let manifest = files.iter().find(|name| name.ends_with(".json")).unwrap();
    let manifest_hash = manifest.trim_end_matches(".json");
    let site_hash = fs::read_dir(output.join("site"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name()
        .into_string()
        .unwrap();
    let page = format!("PREVIEW_ROOT/site/{site_hash}/index.html?changes={manifest_hash}");
    assert!(comment.contains(&format!("**[Review them]({page})**")));
    assert!(comment.contains(&format!(
        "- [`sample`]({page}&painting=sample) was added (4 pixels)\n"
    )));
    assert!(
        output
            .join("site")
            .join(&site_hash)
            .join("index.html")
            .exists()
    );
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(output.join(manifest)).unwrap()).unwrap();
    let painting = &manifest.paintings[0];
    assert_eq!(painting.status, Status::Added);
    let frame = &painting.frames[0];
    assert_eq!(frame.before, None);
    let after = frame.after.as_ref().unwrap();
    assert_eq!((after.width, after.height), (2, 2));
    assert_eq!(after.thumbnail, after.image, "a small image is its own thumbnail");
    assert_eq!(painting.changed_pixels, 4);
    assert!(files.contains(&format!("{}.webp", after.image)));
    let webp = fs::read(output.join(format!("{}.webp", after.image))).unwrap();
    let decoded = image::load_from_memory_with_format(&webp, image::ImageFormat::WebP)
        .unwrap()
        .to_rgba8();
    assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0, 255], "the image is lossless");
    assert!(comment.contains(&format!(
        "<a href=\"PREVIEW_ROOT/{0}.webp\"><img alt=\"After\" src=\"PREVIEW_ROOT/{0}.webp\"></a>",
        after.image
    )));
    assert!(comment.contains("<th>Before</th><th>After</th><th>Changes</th>"));
    assert_eq!(comment.matches("<details>").count(), 1);
    assert_eq!(comment.matches("</details>").count(), 1);
    assert!(!comment.contains("<details open"));
    let disclosure = comment.find("<details>").unwrap();
    assert!(comment.find("**[Review them]").unwrap() < disclosure);
    assert!(comment.ends_with("</details>\n"));
    fs::remove_dir_all(root).unwrap();
}
