use super::*;

use std::sync::Arc;

use be_block_client::Peer;
use be_block_store::FileStore;

use crate::terminal_host::{Host, HostOptions};

#[tokio::test]
async fn a_terminal_host_keeps_its_standing_in_its_status_file() {
    let status = std::env::temp_dir().join(format!("be3-terminal-status-{}", Uuid::new_v4()));
    let mut host = Host::new(HostOptions {
        command: vec!["sh".to_owned()],
        machine: "sandbox".to_owned(),
        user: "user".to_owned(),
        status: Some(status.clone()),
    })
    .unwrap();
    let refused = async || Err::<(Arc<Peer<FileStore>>, Uuid), String>("the token was revoked".to_owned());
    let ended = host
        .run(refused, &sources(), std::future::pending::<()>())
        .await;
    assert_eq!(ended, Err("the token was revoked".to_owned()));
    assert_eq!(
        std::fs::read_to_string(&status).unwrap(),
        "offline the token was revoked\n",
        "a host that could not connect says so"
    );
    let _ = std::fs::remove_file(&status);
}
