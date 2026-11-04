use super::*;
use crate::limits::*;
use rusqlite::params;

fn refused<T: std::fmt::Debug>(result: Result<T, ServerError>, code: ErrorCode) {
    assert!(
        matches!(&result, Err(ServerError::Refused(found, _)) if *found == code),
        "{result:?}"
    );
}

#[tokio::test]
async fn what_one_account_keeps_on_the_server_is_bounded() {
    let directory = std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4()));
    let store = ServerStore::open(&directory).unwrap();
    let long = "x".repeat(MAX_NAME_BYTES + 1);
    refused(
        store.create_local_profile(&long).await,
        ErrorCode::InvalidRequest,
    );
    let (profile, _) = store.create_local_profile("Owner").await.unwrap();
    let account = profile.account;
    refused(
        store.create_organization(account, &long).await,
        ErrorCode::InvalidRequest,
    );

    let organizations: Vec<Uuid> = {
        let mut made = Vec::new();
        for index in 0..MAX_ORGANIZATIONS_PER_ACCOUNT {
            let organization = store
                .create_organization(account, &format!("Company {index}"))
                .await
                .unwrap();
            made.push(organization.id);
        }
        made
    };
    refused(
        store.create_organization(account, "One too many").await,
        ErrorCode::OverQuota,
    );
    let organization = organizations[0];
    let administrator = Identity {
        account,
        organization,
        role: OrganizationRole::Administrator,
    };
    refused(
        store
            .create_agent(
                account,
                organization,
                &long,
                [0; 32],
                Vec::new(),
                Vec::new(),
            )
            .await,
        ErrorCode::InvalidRequest,
    );

    {
        let database = store.database().lock().await;
        let transaction = database.unchecked_transaction().unwrap();
        for index in 0..MAX_CONTACT_LINKS_PER_ACCOUNT {
            transaction
                .execute(
                    "INSERT INTO contact_links (id, owner_id, card, code) VALUES (?1, ?2, x'', x'')",
                    params![Uuid::new_v4().to_string(), account.to_string()],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO invitations (id, organization_id, email, role, invited_by)
                     VALUES (?1, ?2, ?3, 'editor', ?4)",
                    params![
                        Uuid::new_v4().to_string(),
                        organization.to_string(),
                        format!("{index}@example.com"),
                        account.to_string()
                    ],
                )
                .unwrap();
        }
        for epoch in 1..=MAX_ESCROWS_PER_ORGANIZATION {
            transaction
                .execute(
                    "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
                     VALUES (?1, ?2, ?1, 'escrow', ?3, 0)",
                    params![organization.to_string(), Uuid::new_v4().to_string(), epoch],
                )
                .unwrap();
        }
        // Before anything loads the organization's graph, which would not see these rows.
        for _ in 0..MAX_BLOCKS_PER_ACCOUNT {
            transaction
                .execute(
                    "INSERT INTO blocks (organization_id, id, content_type, author, parent_kind)
                     VALUES (?1, ?2, ?3, ?4, 0)",
                    params![
                        organization.to_string(),
                        Uuid::new_v4().to_string(),
                        CONTENT.to_string(),
                        account.to_string()
                    ],
                )
                .unwrap();
        }
        for _ in 0..MAX_CONTACTS_PER_ACCOUNT {
            let contact = Uuid::new_v4().to_string();
            transaction
                .execute(
                    "INSERT INTO accounts (id, email, display_name, password_hash)
                     VALUES (?1, ?1, 'Contact', '!')",
                    [&contact],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO contacts (account_id, contact_id) VALUES (?1, ?2)",
                    params![account.to_string(), contact],
                )
                .unwrap();
        }
        transaction.commit().unwrap();
    }
    refused(
        store
            .create_contact_link(account, Uuid::new_v4(), Vec::new(), Vec::new(), None)
            .await,
        ErrorCode::OverQuota,
    );
    refused(
        store
            .create_contact_link(
                account,
                Uuid::new_v4(),
                vec![0; MAX_SEALED_BYTES + 1],
                Vec::new(),
                None,
            )
            .await,
        ErrorCode::InvalidRequest,
    );
    let invite = |email: &'static str| {
        store.invite(
            account,
            Uuid::new_v4(),
            organization,
            email,
            OrganizationRole::Editor,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    refused(invite("new@example.com").await, ErrorCode::OverQuota);
    invite("0@example.com")
        .await
        .expect("inviting someone again replaces their invitation");
    refused(
        store
            .create_escrow(
                administrator,
                be_block_protocol::EscrowKey {
                    id: Uuid::new_v4(),
                    epoch: MAX_ESCROWS_PER_ORGANIZATION as u64 + 1,
                    public: [0; 32],
                    signing: [0; 32],
                    signature: Vec::new(),
                },
                KeySet::default(),
            )
            .await,
        ErrorCode::OverQuota,
    );
    refused(
        store
            .create_agent(
                account,
                organization,
                "Agent",
                [0; 32],
                Vec::new(),
                Vec::new(),
            )
            .await,
        ErrorCode::OverQuota,
    );

    let block = |metadata: Vec<u8>| {
        store.create_block(
            administrator,
            Uuid::new_v4(),
            CONTENT,
            BlockParent::Detached,
            metadata,
            Vec::new(),
            Uuid::new_v4(),
            KeySet::default(),
        )
    };
    refused(
        block(vec![0; MAX_METADATA_BYTES + 1]).await,
        ErrorCode::InvalidRequest,
    );
    refused(block(Vec::new()).await, ErrorCode::OverQuota);

    let _ = std::fs::remove_dir_all(directory);
}
