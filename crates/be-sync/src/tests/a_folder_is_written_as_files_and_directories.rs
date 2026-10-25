use super::*;
use image_block_types::ImageFile;
use workspace_ui_types::FileHeader;

#[tokio::test]
async fn a_folder_is_written_as_files_and_directories() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let docs = folder(&peer, code, "docs").await;
    text_block(&peer, docs, "guide.md", "# Guide\n\nRead me.\n").await;
    text_block(&peer, code, "main.rs", "fn main() {}\n").await;
    let png = vec![0x89, b'P', b'N', b'G', 0, 1, 2, 3];
    block(
        &peer,
        code,
        "logo.png",
        &ImageFile::from_file("logo.png", png.clone()),
    )
    .await;
    let painting = vec![0, 159, 146, 150, 255];
    block(
        &peer,
        docs,
        "frame.paint",
        &FileContent::new(
            FileHeader {
                source_name: "frame.paint".into(),
                media_type: "application/octet-stream".into(),
            },
            painting.clone(),
        ),
    )
    .await;
    block(&peer, code, "Sketch", &CanvasContent::default()).await;
    text_block(&peer, code, "a/b", "slashes").await;
    text_block(&peer, code, "CON", "a device").await;
    text_block(&peer, code, "notes", "lower").await;
    text_block(&peer, code, "Notes", "upper").await;

    let checkout = harness.checkout("checkout");
    let report = sync(&peer, &checkout, code).await;

    assert_eq!(read(&checkout, "docs/guide.md"), "# Guide\n\nRead me.\n");
    assert_eq!(read(&checkout, "main.rs"), "fn main() {}\n");
    assert_eq!(std::fs::read(checkout.join("logo.png")).unwrap(), png);
    assert_eq!(
        std::fs::read(checkout.join("docs/frame.paint")).unwrap(),
        painting
    );
    for absent in ["Sketch", "CON", "notes", "Notes"] {
        assert!(!checkout.join(absent).exists(), "{absent} was written");
    }
    let errors = read(&checkout, crate::ERRORS_FILE);
    for named in ["a/b", "CON", "notes", "Notes"] {
        assert!(
            errors.contains(named),
            "{named} is not explained:\n{errors}"
        );
    }
    assert!(!errors.contains("Sketch"), "{errors}");
    let unexported = read(&checkout, ".be/unexported.txt");
    assert!(
        unexported.contains("Sketch") && unexported.contains("no plain-file form"),
        "{unexported}"
    );
    assert_eq!(report.skipped.len(), 4, "{report}");
    assert_eq!(report.unexported, 1, "{report}");
    assert_eq!(report.created, 0, "{report}");

    let again = sync(&peer, &checkout, code).await;
    assert!(!again.changed(), "{again}");
    assert_eq!(children(&peer, code).await.len(), 8);
    harness.stop().await;
}
