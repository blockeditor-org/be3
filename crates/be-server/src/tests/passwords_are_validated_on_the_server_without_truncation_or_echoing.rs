use super::*;

#[tokio::test]
async fn passwords_are_validated_on_the_server_without_truncation_or_echoing() {
    let harness = Harness::start().await;
    let mut browser = Browser::default();
    let secret = "a password that must not come back";
    let long_name = "A".repeat(201);
    let (status, page) = browser
        .submit(
            &harness,
            "/auth/register",
            "/auth/register",
            &[
                ("email", "valid@example.com"),
                ("display_name", &long_name),
                ("password", secret),
                ("confirmation", secret),
            ],
        )
        .await;
    assert_eq!(status, 400);
    assert!(page.contains(&long_name));
    assert!(page.contains("Enter a name of at most 200 bytes."));
    assert!(!page.contains(secret));
    let (_, page) = browser
        .submit(
            &harness,
            "/auth/register",
            "/auth/register",
            &[
                ("email", "bad address"),
                ("display_name", "Ada"),
                ("password", secret),
                ("confirmation", secret),
            ],
        )
        .await;
    assert!(page.contains("Enter a valid email address"));
    assert!(page.contains("bad address"));
    let (_, page) = browser
        .submit(
            &harness,
            "/auth/register",
            "/auth/register",
            &[
                ("email", "valid@example.com"),
                ("display_name", "Ada"),
                ("password", secret),
                ("confirmation", "different"),
            ],
        )
        .await;
    assert!(page.contains("The passwords do not match."));
    assert!(!page.contains(secret));
    for constraint in ["required=", "maxlength=", "minlength=", "pattern="] {
        assert!(!page.contains(constraint));
    }
    assert!(page.contains("autocomplete=\"new-password\""));
    assert!(harness.mail.0.lock().unwrap().is_empty());
}
