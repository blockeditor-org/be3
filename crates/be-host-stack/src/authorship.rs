//! Apps' `sign` and `verify` (plans/authorship.md): the person's signing key never leaves the
//! worker, and who signed what is answered from the keys the device knows.

use std::sync::Arc;

use be_block_client::Peer;
pub use be_block_client::Signer;
use uuid::Uuid;

use crate::worker::{Command, Store, Tasks};

type Reply<T> = crate::replies::Reply<Result<T, String>>;

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) enum AuthorshipCommand {
    Sign {
        block: Uuid,
        bytes: Vec<u8>,
        reply: Reply<Vec<u8>>,
    },
    Verify {
        account: Uuid,
        block: Uuid,
        bytes: Vec<u8>,
        signature: Vec<u8>,
        arrived: Option<i64>,
        reply: Reply<crate::replies::Signed>,
    },
}

/// Signs `bytes` the person wrote in `block`.
pub fn sign(
    block: Uuid,
    bytes: Vec<u8>,
    reply: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    crate::send(Command::Authorship(AuthorshipCommand::Sign {
        block,
        bytes,
        reply: crate::replies::expect(reply),
    }));
}

/// Who of `account` signed `bytes` in `block`, which the server took at `arrived` if the
/// record carries that stamp (`Peer::verify_authored`).
pub fn verify(
    account: Uuid,
    block: Uuid,
    bytes: Vec<u8>,
    signature: Vec<u8>,
    arrived: Option<i64>,
    reply: impl FnOnce(Result<Signer, String>) + Send + 'static,
) {
    crate::send(Command::Authorship(AuthorshipCommand::Verify {
        account,
        block,
        bytes,
        signature,
        arrived,
        reply: crate::replies::expect(move |signer: Result<crate::replies::Signed, String>| {
            reply(signer.map(|crate::replies::Signed(signer)| signer));
        }),
    }));
}

pub(crate) fn run(peer: &Arc<Peer<Store>>, tasks: &Tasks, command: AuthorshipCommand) {
    match command {
        AuthorshipCommand::Sign {
            block,
            bytes,
            reply,
        } => reply.send(Ok(peer.sign_authored(block, &bytes))),
        AuthorshipCommand::Verify {
            account,
            block,
            bytes,
            signature,
            arrived,
            reply,
        } => {
            let peer = Arc::clone(peer);
            tasks.spawn(async move {
                let verified = peer
                    .verify_authored(account, block, &bytes, &signature, arrived)
                    .await;
                reply.send(
                    verified
                        .map(crate::replies::Signed)
                        .map_err(|error| error.to_string()),
                );
                crate::changes::wake();
            });
        }
    }
}
