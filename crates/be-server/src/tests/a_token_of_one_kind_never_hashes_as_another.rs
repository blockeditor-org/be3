use super::*;
use crate::store::{TokenKind, hash_token};

/// The server keeps only secrets' hashes, each kind under its own domain, so a secret of one
/// kind (a browser's attempt, say) never matches a row kept for another (a session).
#[test]
fn a_token_of_one_kind_never_hashes_as_another() {
    let kinds = [
        TokenKind::Session,
        TokenKind::Attempt,
        TokenKind::PublicAttempt,
        TokenKind::Code,
        TokenKind::ProviderState,
        TokenKind::ProviderStart,
        TokenKind::MailLink,
        TokenKind::LoginLink,
        TokenKind::Invite,
    ];
    let token = "one secret";
    let hashes: std::collections::HashSet<String> =
        kinds.iter().map(|kind| hash_token(*kind, token)).collect();
    assert_eq!(hashes.len(), kinds.len());
    let bare = Hash::from_bytes(<sha2::Sha256 as sha2::Digest>::digest(token).into()).to_hex();
    assert!(!hashes.contains(&bare));
}
