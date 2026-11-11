//! Apps' `sign` and `verify` (plans/authorship.md): the person's signing key never leaves the
//! worker, and who signed what is answered from the keys the device knows.

use std::sync::Arc;

use be_block_client::Peer;
pub use be_block_client::Signer;
use uuid::Uuid;

use crate::worker::{Command, Store, Tasks};

type Reply<T> = Box<dyn FnOnce(Result<T, String>) + Send>;

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
        reply: Reply<Signer>,
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
        reply: Box::new(reply),
    }));
}

/// Who of `account` signed `bytes` in `block` (`Peer::verify_authored`).
pub fn verify(
    account: Uuid,
    block: Uuid,
    bytes: Vec<u8>,
    signature: Vec<u8>,
    reply: impl FnOnce(Result<Signer, String>) + Send + 'static,
) {
    crate::send(Command::Authorship(AuthorshipCommand::Verify {
        account,
        block,
        bytes,
        signature,
        reply: Box::new(reply),
    }));
}

pub(crate) fn run(peer: &Arc<Peer<Store>>, tasks: &Tasks, command: AuthorshipCommand) {
    match command {
        AuthorshipCommand::Sign {
            block,
            bytes,
            reply,
        } => reply(Ok(peer.sign_authored(block, &bytes))),
        AuthorshipCommand::Verify {
            account,
            block,
            bytes,
            signature,
            reply,
        } => {
            let peer = Arc::clone(peer);
            tasks.spawn(async move {
                let verified = peer
                    .verify_authored(account, block, &bytes, &signature)
                    .await;
                reply(verified.map_err(|error| error.to_string()));
                be_host_base::host::wake();
            });
        }
    }
}
