use super::*;

/// Each message shows who signed it as the host verified its entry: a key not verified, one
/// that changed, an old one, or none of its author's.
#[test]
fn a_message_shows_who_signed_it() {
    let tom = Uuid::from_u128(0x7000_0000_0000_4000_8000_0000_0000_0007);
    let ana = Uuid::from_u128(0xa4a0_0000_0000_4000_8000_0000_0000_0008);
    let eve = Uuid::from_u128(0xe7e0_0000_0000_4000_8000_0000_0000_0009);
    let mut chat = Harness::trusting(
        &[
            (ROSA, "Verified by her card"),
            (tom, "From someone not verified"),
            (ana, "Her key changed"),
            (eve, "Signed with an old key"),
            (ME, "Forged in my name"),
        ],
        &[
            (tom, Signer::Unverified),
            (ana, Signer::Changed),
            (eve, Signer::Replaced),
            (ME, Signer::Bad),
        ],
    );
    let rows: Vec<String> = (0..5).map(|index| chat.row(chat.seq(index))).collect();
    assert!(!rows[0].contains("verified"), "{}", rows[0]);
    assert!(rows[1].contains("not verified"), "{}", rows[1]);
    assert!(rows[2].contains("key changed"), "{}", rows[2]);
    assert!(rows[3].contains("sent with an old key"), "{}", rows[3]);
    assert!(rows[4].contains("forged"), "{}", rows[4]);
    chat.snapshot("a_message_shows_whether_its_author_signed_it");
}
