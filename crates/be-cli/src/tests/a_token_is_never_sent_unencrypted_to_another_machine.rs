use crate::session::{Settings, connect};

#[tokio::test]
async fn a_token_is_never_sent_unencrypted_to_another_machine() {
    let settings = Settings {
        server: "http://blocks.example".to_owned(),
        workspace: None,
        token: "secret".to_owned(),
        phrase: String::new(),
        authorization: None,
    };
    let error = connect(&settings).await.err().unwrap();
    assert!(error.contains("not https"), "{error}");
}
