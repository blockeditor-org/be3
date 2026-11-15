use super::*;
use crate::fake_provider::Grant;

#[tokio::test]
async fn a_provider_token_with_the_wrong_audience_or_a_strangers_signature_is_refused() {
    let (harness, provider) = with_provider(true).await;
    for grant in [
        Grant {
            audience: Some("another-client".into()),
            ..Grant::new("ada")
        },
        Grant {
            stranger_signs: true,
            ..Grant::new("ada")
        },
        Grant {
            unsigned: true,
            ..Grant::new("ada")
        },
        Grant {
            expired: true,
            ..Grant::new("ada")
        },
        // The code is redeemed only with the verifier behind its PKCE challenge.
        Grant {
            challenge: Some("another-challenge".into()),
            ..Grant::new("ada")
        },
    ] {
        let mut browser = Browser::default();
        let (status, response) = browser
            .provider_sign_in(&harness, &provider, "test", "", grant)
            .await;
        assert_eq!(status, 400, "{response}");
        assert!(browser.cookie.is_empty());
    }
    assert_eq!(accounts(&harness), 0);
    let pending: i64 = metadata_database(&harness.directory)
        .query_row("SELECT count(*) FROM oidc_pending", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pending, 0, "a refused token left a registration waiting");
}
