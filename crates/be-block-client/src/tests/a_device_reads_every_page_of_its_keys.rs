use super::*;
use be_type_text::{TextBlock, TextContent};

/// The server answers keys a page at a time; a device signing in reads every page, so it opens
/// blocks whose keys came on any of them.
#[tokio::test]
async fn a_device_reads_every_page_of_its_keys() {
    let harness = Harness::configured(be_server::ServerConfig {
        key_page_bytes: 1024,
        ..be_server::ServerConfig::OPEN
    })
    .await;
    let author = harness.owner("keeper@example.com").await;
    let workspace = workspace(&author, "Keys").await;
    let mut written = Vec::new();
    for index in 0..30 {
        let note = author
            .create::<TextContent>(BlockParent::Block(workspace))
            .await
            .unwrap();
        let text = TextBlock::of(&format!("note {index}"));
        author.save(note, &text, None).await.unwrap();
        written.push((note, text.encode()));
    }
    let page = crate::ask(be_block_protocol::client::keys::get_keys(
        &author.connection(),
        None,
        None,
    ))
    .await
    .unwrap();
    assert!(page.next.is_some(), "the keys fit one page");

    let reader = harness.second(&author).await;
    let asked: Vec<_> = written.iter().map(|(note, _)| (*note, None)).collect();
    let read = reader.read_contents(&asked).await.unwrap();
    for ((note, bytes), read) in written.iter().zip(read) {
        assert_eq!(read.summary.id, *note);
        assert_eq!(
            read.stored.map(|stored| stored.bytes).as_ref(),
            Some(bytes),
            "a block whose key came on a later page did not open"
        );
    }

    harness.stop().await;
}
