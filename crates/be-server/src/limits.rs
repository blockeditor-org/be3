//! What one account or one connection may make the server keep. Each bounds memory, rows or
//! per-call work an attacker could otherwise grow without end, and sits well above what a person
//! or a host uses. Objects are bounded separately (`store::MAX_OBJECT_BYTES`, the organization's
//! quota), as are frames (`be_block_protocol::MAX_FRAME_BYTES`).

use be_block_protocol::{ErrorCode, MAX_FRAME_BYTES};

use crate::ServerError;

/// Blocks one account may have authored in one organization. Every block is held in memory in
/// its organization's graph.
pub const MAX_BLOCKS_PER_ACCOUNT: usize = 250_000;
/// Company organizations one account may have created.
pub const MAX_ORGANIZATIONS_PER_ACCOUNT: i64 = 50;
/// Contact links one account may have open (made and not deleted); invitations are bounded by
/// `MAX_INVITATIONS_PER_ORGANIZATION` instead.
pub const MAX_CONTACT_LINKS_PER_ACCOUNT: i64 = 1_000;
/// Contacts one account may have.
pub const MAX_CONTACTS_PER_ACCOUNT: i64 = 10_000;
/// Invitations an organization may have outstanding.
pub const MAX_INVITATIONS_PER_ORGANIZATION: i64 = 1_000;
/// Escrow keys an organization may make; each rotation makes one.
pub const MAX_ESCROWS_PER_ORGANIZATION: i64 = 1_000;

/// Push endpoints (devices) one account keeps per organization; registering another drops the
/// oldest.
pub const MAX_PUSH_ENDPOINTS: i64 = 32;
/// Notifications one account may send a minute, over all blocks.
pub const MAX_NOTIFICATIONS_PER_MINUTE: u32 = 60;
/// Notifications one account may send a minute about one block; more waiting for one device
/// replace each other (`Pusher::queue`).
pub const MAX_NOTIFICATIONS_PER_BLOCK: u32 = 10;
/// Accounts one notification may mention.
pub const MAX_MENTIONS: usize = 1_024;

/// Live sessions one connection may be joined to at once.
pub const MAX_SESSIONS_PER_CONNECTION: usize = 1_024;
/// Listings one connection may hold at once: folders, references and backlinks open. A block's
/// own listing (`Listing::Block`), which nothing unlists, does not count (`watch.rs`, `counts`).
pub const MAX_LISTINGS_PER_CONNECTION: usize = 65_536;
/// Streams (watches, session events, change feeds) one connection may have open at once.
pub const MAX_STREAMS_PER_CONNECTION: usize = 16_384;

/// An organization name, an agent's name, a local profile's display name: the portal's limit
/// on display names.
pub const MAX_NAME_BYTES: usize = 200;
/// A block's metadata. It is kept in memory in the graph and sent in every summary, so it is
/// small next to a frame: a listing of many blocks must still fit one.
pub const MAX_METADATA_BYTES: usize = 64 * 1024;
/// A sealed blob the server keeps for a client: contact cards and their sealed copies, invitation
/// payloads, pinned escrows, pending grants.
pub const MAX_SEALED_BYTES: usize = 64 * 1024;
/// Accounts named in one `get-signing-keys`: more than a chat shows at once.
pub const MAX_SIGNING_KEYS_ASKED: usize = 1_024;
/// A pairing message between an account's connections.
pub const MAX_PAIRING_BYTES: usize = 64 * 1024;
/// A message relayed in a live session. Replicas relay operations of at most 1 MiB and send a
/// larger one by the part they put its bytes as (`LARGEST_RELAYED_OPERATION` in
/// be-sdk-type-guest); a catch-up snapshot carries the operations since the last seal, so this
/// leaves room for both.
pub const MAX_RELAY_BYTES: usize = MAX_FRAME_BYTES / 2;
/// The objects one commit of a search index may hold. Every viewer's search reads the index,
/// and any editor of its root may write it, so it is bounded well above an index of a gigabyte
/// of text (plans/search.md, "Scale").
pub const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub use be_block_commit::MAX_COMMIT_AHEAD;

pub(crate) fn too_many(what: &str, limit: impl std::fmt::Display) -> ServerError {
    ServerError::Refused(ErrorCode::OverQuota, format!("{what}: at most {limit}"))
}

pub(crate) fn at_most(what: &str, bytes: usize, limit: usize) -> Result<(), ServerError> {
    match bytes <= limit {
        true => Ok(()),
        false => Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            format!("{what} may be at most {limit} bytes, not {bytes}"),
        )),
    }
}

/// Checks a name, trimmed, is present and fits `MAX_NAME_BYTES`.
pub(crate) fn name<'a>(what: &str, name: &'a str) -> Result<&'a str, ServerError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ServerError::Refused(
            ErrorCode::InvalidRequest,
            format!("{what} is required"),
        ));
    }
    at_most(what, name.len(), MAX_NAME_BYTES)?;
    Ok(name)
}
