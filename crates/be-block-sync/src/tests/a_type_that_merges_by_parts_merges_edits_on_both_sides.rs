use be_sdk_type_testing::sections::Sections;

use super::*;

const LINES: [&str; 10] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
];

fn lines(changed: &[(usize, &str)]) -> String {
    let mut lines = LINES;
    for (index, text) in changed {
        lines[*index] = text;
    }
    lines.map(|line| format!("{line}\n")).concat()
}

async fn sections_of(peer: &Peer<MemoryStore>, id: Uuid) -> Sections {
    let head = peer.local_head(id).expect("the block has a head");
    let commit = peer.load_commit(head).await.unwrap();
    Sections::decode(&peer.read_manifest(&commit.manifest).await.unwrap()).unwrap()
}

#[tokio::test]
async fn a_type_that_merges_by_parts_merges_edits_on_both_sides() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let list = block(&peer, code, "list.sections", &Sections::of(&LINES)).await;
    let checkout = harness.checkout("checkout");
    sync(&peer, &checkout, code).await;
    assert_eq!(read(&checkout, "list.sections"), lines(&[]));

    write(&checkout, "list.sections", &lines(&[(2, "TWO")]));
    let mut remote = sections_of(&peer, list).await;
    remote.set(5, "FIVE");
    peer.save(list, &remote, peer.local_head(list))
        .await
        .unwrap();
    let report = sync(&peer, &checkout, code).await;
    assert!(report.conflicts.is_empty(), "{report}");
    let merged = lines(&[(2, "TWO"), (5, "FIVE")]);
    assert_eq!(read(&checkout, "list.sections"), merged);
    let content = sections_of(&peer, list).await;
    assert_eq!(content.section(2), Some(&b"TWO"[..]));
    assert_eq!(content.section(5), Some(&b"FIVE"[..]));

    let again = sync(&peer, &checkout, code).await;
    assert!(!again.changed(), "{again}");
    harness.stop().await;
}
