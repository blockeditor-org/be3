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
        store.register("long@example.com", &long).await,
        ErrorCode::InvalidRequest,
    );
    let (profile, _) = store.register("owner@example.com", "Owner").await.unwrap();
    let account = profile.account;
    refused(
        signed_organization(&store, account, &long).await,
        ErrorCode::InvalidRequest,
    );

    let organizations: Vec<Uuid> = {
        let mut made = Vec::new();
        for index in 0..MAX_ORGANIZATIONS_PER_ACCOUNT {
            let organization = signed_organization(&store, account, &format!("Company {index}"))
                .await
                .unwrap();
            made.push(organization.id);
        }
        made
    };
    refused(
        signed_organization(&store, account, "One too many").await,
        ErrorCode::OverQuota,
    );
    let organization = organizations[0];
    let administrator = Identity {
        account,
        organization,
        role: OrganizationRole::Administrator,
        bound: None,
    };
    refused(
        store
            .create_agent(
                account,
                organization,
                agent_id(account, &unused_keys(7)),
                &long,
                [7; 32],
                [7; 32],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
                None,
            )
            .await,
        ErrorCode::InvalidRequest,
    );

    let (giver, given) = (Uuid::new_v4(), Uuid::new_v4());
    store
        .with_database(move |_, database| {
        let transaction = be_block_sqlite_schema::write_transaction(database).unwrap();
        for index in 0..MAX_CONTACT_LINKS_PER_ACCOUNT {
            transaction
                .execute(
                    "INSERT INTO links (id, owner_id, card) VALUES (?1, ?2, x'')",
                    params![Uuid::new_v4().to_string(), account.to_string()],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO links (id, owner_id, card, invite_organization,
                                        invite_created, invite_expires)
                     VALUES (?1, ?2, x'', ?3, ?4, ?5)",
                    params![
                        Uuid::new_v4().to_string(),
                        account.to_string(),
                        organization.to_string(),
                        index,
                        i64::MAX
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
                    "INSERT INTO blocks (organization_id, id, content_type, author, parent_kind, payer)
                     VALUES (?1, ?2, ?3, ?4, 0, ?4)",
                    params![
                        organization.to_string(),
                        Uuid::new_v4().to_string(),
                        CONTENT.to_string(),
                        account.to_string()
                    ],
                )
                .unwrap();
        }
        // A block someone else owns, which they would give to the account.
        transaction
            .execute(
                "INSERT INTO blocks (organization_id, id, content_type, author, parent_kind, payer)
                 VALUES (?1, ?2, ?3, ?4, 0, ?4)",
                params![
                    organization.to_string(),
                    given.to_string(),
                    CONTENT.to_string(),
                    giver.to_string()
                ],
            )
            .unwrap();
        // The giver is a contact of the account, so the transfer below is refused for the
        // account's bound alone: to anyone else, a transfer is refused before it.
        transaction
            .execute(
                "INSERT INTO accounts (id, email, display_name) VALUES (?1, ?1, 'Giver')",
                [giver.to_string()],
            )
            .unwrap();
        transaction
            .execute(
                "INSERT INTO contacts (account_id, contact_id) VALUES (?1, ?2)",
                params![giver.to_string(), account.to_string()],
            )
            .unwrap();
        for _ in 0..MAX_CONTACTS_PER_ACCOUNT {
            let contact = Uuid::new_v4().to_string();
            transaction
                .execute(
                    "INSERT INTO accounts (id, email, display_name) VALUES (?1, ?1, 'Contact')",
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
        Ok(())
        })
        .await
        .unwrap();
    refused(
        store
            .create_link(
                account,
                Uuid::new_v4(),
                Vec::new(),
                be_block_protocol::LinkKind::Contact(None),
                None,
            )
            .await,
        ErrorCode::OverQuota,
    );
    refused(
        store
            .create_link(
                account,
                Uuid::new_v4(),
                vec![0; MAX_SEALED_BYTES + 1],
                be_block_protocol::LinkKind::Contact(None),
                None,
            )
            .await,
        ErrorCode::InvalidRequest,
    );
    let invite = || {
        store.create_link(
            account,
            Uuid::new_v4(),
            Vec::new(),
            be_block_protocol::LinkKind::Invitation(be_block_protocol::Invitation {
                organization,
                escrow: Vec::new(),
            }),
            // Refused for the bound before the record is read.
            Some(Vec::new()),
        )
    };
    refused(invite().await, ErrorCode::OverQuota);
    refused(
        store
            .create_escrow(
                administrator,
                be_block_protocol::EscrowKey {
                    id: Uuid::new_v4(),
                    epoch: MAX_ESCROWS_PER_ORGANIZATION as u64 + 1,
                    public: [0; 32],
                    head: [0; 32],
                    maker: Uuid::nil(),
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
                agent_id(account, &unused_keys(7)),
                "Agent",
                [7; 32],
                [7; 32],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                None,
                None,
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
            false,
            Vec::new(),
            Vec::new(),
        )
    };
    refused(
        block(vec![0; MAX_METADATA_BYTES + 1]).await,
        ErrorCode::InvalidRequest,
    );
    refused(block(Vec::new()).await, ErrorCode::OverQuota);
    let secret = be_block_keys::RecoveryPhrase::generate().secret();
    let transfer = be_block_keys::Entry::sign(
        &secret,
        giver.into_bytes(),
        given.into_bytes(),
        None,
        true,
        be_block_keys::Record::Owner(be_block_keys::Owner {
            account: account.into_bytes(),
            identity: unused_keys(9),
        }),
    );
    refused(
        store
            .set_access(
                Identity {
                    account: giver,
                    organization,
                    role: OrganizationRole::Editor,
                    bound: None,
                },
                given,
                account,
                be_block_graph::Access::Manage,
                Some(transfer.encode()),
            )
            .await,
        ErrorCode::OverQuota,
    );

    let _ = std::fs::remove_dir_all(directory);
}
