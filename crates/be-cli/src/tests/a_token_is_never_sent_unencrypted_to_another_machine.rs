use crate::session::{Settings, connect, connect_live};

#[tokio::test]
async fn a_token_is_never_sent_unencrypted_to_another_machine() {
    let settings = Settings {
        server: "http://blocks.example".to_owned(),
        organization: None,
        token: "secret".to_owned(),
        phrase: String::new(),
        authorization: None,
        cache: std::env::temp_dir(),
    };
    let error = connect(&settings).await.err().unwrap();
    assert!(error.contains("not https"), "{error}");
    let error = connect_live(&settings).await.err().unwrap();
    assert!(error.contains("not https"), "{error}");
}
