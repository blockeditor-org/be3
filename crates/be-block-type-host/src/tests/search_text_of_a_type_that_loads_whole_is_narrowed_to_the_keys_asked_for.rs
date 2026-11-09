use be_sdk_type_api::SearchDocument;
use be_type_text::{TextBlock, TextContent};

use super::*;

#[test]
fn search_text_of_a_type_that_loads_whole_is_narrowed_to_the_keys_asked_for() {
    with_test_types(async |peer, shared, mut replicas| {
        let block = peer
            .create::<TextContent>(BlockParent::Root)
            .await
            .expect("the text is made");
        let text = "Oat milk\n\nand rye bread.\n";
        let manifest = peer
            .write_content(
                block,
                TextContent::CONTENT_TYPE,
                TextBlock::of(text).parts(),
            )
            .await
            .expect("the text is written");
        let mut search = async |keys: Option<Vec<String>>| {
            replicas
                .search_text(
                    (&peer, &shared),
                    TextContent::CONTENT_TYPE,
                    &manifest,
                    keys,
                    &[],
                )
                .await
                .expect("the text answers")
        };

        let whole = search(None).await;
        assert!(
            !crate::kind_of(TextContent::CONTENT_TYPE)
                .expect("text is known")
                .first_part,
            "text loads whole"
        );
        assert_eq!(
            whole.documents,
            vec![SearchDocument {
                key: String::new(),
                source: None,
                name: None,
                body: Some(text.to_owned()),
            }],
            "the whole commit is handed over, and its text is one document"
        );
        assert_eq!(whole.read, Vec::new(), "a type handed its whole commit reads nothing");
        assert_eq!(search(Some(vec![String::new()])).await.documents, whole.documents);
        assert_eq!(
            search(Some(vec!["other".to_owned()])).await.documents,
            Vec::new(),
            "the host answers only the keys asked for"
        );
    });
}
