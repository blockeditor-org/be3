use super::*;

#[tokio::test]
async fn edits_on_both_sides_merge_and_clashes_keep_both() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let note = text_block(&peer, code, "note.md", "one\ntwo\nthree\nfour\nfive\n").await;
    let checkout = harness.checkout("checkout");
    sync(&peer, &checkout, code).await;

    write(&checkout, "note.md", "ONE\ntwo\nthree\nfour\nfive\n");
    rewrite(&peer, note, "one\ntwo\nthree\nfour\nFIVE\n").await;
    let report = sync(&peer, &checkout, code).await;
    assert!(report.conflicts.is_empty(), "{report}");
    assert_eq!(read(&checkout, "note.md"), "ONE\ntwo\nthree\nfour\nFIVE\n");
    assert_eq!(text_of(&peer, note).await, "ONE\ntwo\nthree\nfour\nFIVE\n");

    write(&checkout, "note.md", "ONE\nlocal\nthree\nfour\nFIVE\n");
    rewrite(&peer, note, "ONE\nremote\nthree\nfour\nFIVE\n").await;
    let report = sync(&peer, &checkout, code).await;
    assert_eq!(report.conflicts.len(), 1, "{report}");
    let merged = "ONE\n<<<<<<< local\nlocal\n=======\nremote\n>>>>>>> remote\nthree\nfour\nFIVE\n";
    assert_eq!(read(&checkout, "note.md"), merged);
    assert_eq!(text_of(&peer, note).await, merged);

    let again = sync(&peer, &checkout, code).await;
    assert!(!again.changed(), "{again}");
    harness.stop().await;
}
