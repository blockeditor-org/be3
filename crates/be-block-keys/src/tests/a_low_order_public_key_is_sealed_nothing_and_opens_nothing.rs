use super::*;

/// A contact, or an administrator choosing an escrow key, who publishes a low-order point makes
/// every Diffie-Hellman result with it the same whatever the secret, so a seal to or from it
/// would open for anyone (guides/blocks/keys.md, "Keys").
#[test]
fn a_low_order_public_key_is_sealed_nothing_and_opens_nothing() {
    let mine = RecoveryPhrase::generate().secret();
    // The identity and a point of order eight, each with its high bit set too.
    let low_orders = [
        [0u8; 32],
        {
            let mut one = [0u8; 32];
            one[0] = 1;
            one
        },
        [
            0xe0, 0xeb, 0x7a, 0x7c, 0x3b, 0x41, 0xb8, 0xae, 0x16, 0x56, 0xe3, 0xfa, 0xf1, 0x9f,
            0xc4, 0x6a, 0xda, 0x09, 0x8d, 0xeb, 0x9c, 0x32, 0xb1, 0xfd, 0x86, 0x62, 0x05, 0x16,
            0x5f, 0x49, 0xb8, 0x00,
        ],
    ];
    for low in low_orders {
        let to = Party {
            account: [2; 16],
            public: RecoveryPublic(low),
        };
        assert_eq!(
            mine.seal_for([1; 16], &to, b"key", &[7; 32]),
            Err(KeyError::Public),
            "a key was sealed to a low-order point"
        );
        assert_eq!(
            mine.seal_key_change([1; 16], &to, mine.identity()),
            Err(KeyError::Public)
        );
        // A seal claiming to come from a low-order key: whatever its bytes, it never opens.
        let from = Party {
            account: [2; 16],
            public: RecoveryPublic(low),
        };
        let honest = RecoveryPhrase::generate()
            .secret()
            .seal_for(
                [2; 16],
                &Party {
                    account: [1; 16],
                    public: mine.public(),
                },
                b"key",
                &[7; 32],
            )
            .unwrap();
        assert_eq!(
            mine.open_from(&from, [1; 16], b"key", &honest),
            Err(KeyError::Public)
        );
    }
}
