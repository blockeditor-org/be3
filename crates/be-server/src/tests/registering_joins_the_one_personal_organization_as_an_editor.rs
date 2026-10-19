use be_protocol::{OrganizationKind, OrganizationRole};

use super::*;

#[tokio::test]
async fn registering_joins_the_one_personal_organization_as_an_editor() {
    let directory = std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4()));
    let (account, company) = fixture_account(
        &directory,
        "owner@example.com",
        "Owner",
        "correct horse battery",
        "Company",
    )
    .await
    .unwrap();
    let store = ServerStore::open(&directory).unwrap();

    let personal = store.personal_organization().await.unwrap();
    assert_ne!(personal, company);
    assert_eq!(store.personal_organization().await.unwrap(), personal);
    assert_eq!(
        store.join_personal_organization(account).await.unwrap(),
        personal,
        "joining again is harmless"
    );

    let listed = store.list_organizations(account).await.unwrap();
    let kinds: Vec<(Uuid, OrganizationKind, OrganizationRole)> = listed
        .iter()
        .map(|organization| (organization.id, organization.kind, organization.role))
        .collect();
    assert_eq!(
        kinds,
        [
            (
                personal,
                OrganizationKind::Personal,
                OrganizationRole::Editor
            ),
            (
                company,
                OrganizationKind::Company,
                OrganizationRole::Administrator
            ),
        ],
        "the personal organization comes first, and nobody administers it"
    );
    let _ = std::fs::remove_dir_all(&directory);
}
