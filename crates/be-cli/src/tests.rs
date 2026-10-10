use std::path::PathBuf;

use be_block::database::{Database, DatabaseContent};
use be_block::database_schema::{
    DatabaseFieldType, DatabaseRelationOptions, DatabaseSchema, DatabaseSchemaContent,
};
use be_block::{BlockContent, BlockMetadata};
use be_client::{Credentials, Peer, PeerConfig};
use be_graph::BlockParent;
use be_store::{ContentKey, MemoryStore};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

use crate::cli::{self, Command, Invocation};
use crate::tracker::{Body, Condition, Filter, Tracker};

mod a_csv_export_is_imported_as_rows_with_pages;
mod a_pull_request_moves_the_rows_it_names;
mod an_agent_reads_and_writes_the_rows_shared_with_it;
mod an_edited_page_keeps_what_someone_else_changed_meanwhile;
mod arguments_parse_into_commands;
mod rows_are_added_filtered_changed_and_given_pages;

const KEY: [u8; 32] = [9; 32];

struct Harness {
    url: String,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Harness {
    async fn start() -> Self {
        let directory = std::env::temp_dir().join(format!("be-cli-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, receiver) = oneshot::channel();
        let data_dir = directory.clone();
        let handle = tokio::spawn(async move {
            let _ = be_server::serve_with_config(
                listener,
                data_dir,
                be_server::ServerConfig::OPEN,
                receiver,
            )
            .await;
        });
        Self {
            url: format!("http://{address}"),
            directory,
            shutdown: Some(shutdown),
            handle: Some(handle),
        }
    }

    async fn owner(&self) -> Peer<MemoryStore> {
        Peer::connect(
            PeerConfig::new(
                be_client::socket_url(&self.url),
                ContentKey::from_bytes(KEY),
                Credentials::Register {
                    email: "owner@example.com".into(),
                    display_name: "Owner".into(),
                    password: "correct horse battery".into(),
                },
            ),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = handle.await;
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct Board {
    database: Uuid,
    status: Uuid,
    project: Uuid,
}

async fn database(
    peer: &Peer<MemoryStore>,
    name: &str,
    prefix: &str,
    fields: &[(&str, DatabaseFieldType, &[&str])],
) -> (Uuid, Vec<Uuid>) {
    let mut schema = DatabaseSchemaContent::default();
    let mut ids = Vec::new();
    for (field, field_type, options) in fields {
        let (id, edit) = DatabaseSchema::add_field(*field, *field_type);
        schema.apply(&edit);
        for option in *options {
            schema.apply(&DatabaseSchema::add_enum_option(id, *option).1);
        }
        ids.push(id);
    }
    let database = Uuid::new_v4();
    let schema_id = Uuid::new_v4();
    peer.create_block(
        database,
        DatabaseContent::CONTENT_TYPE,
        BlockParent::Root,
        &BlockMetadata::named(name),
    )
    .await
    .unwrap();
    peer.create_block(
        schema_id,
        DatabaseSchemaContent::CONTENT_TYPE,
        BlockParent::Block(database),
        &BlockMetadata::default(),
    )
    .await
    .unwrap();
    peer.save(schema_id, &schema, None).await.unwrap();
    let mut content = DatabaseContent::new(&Database::with_schema(schema_id));
    content.apply(&Database::set_prefix(prefix));
    peer.save(database, &content, None).await.unwrap();
    (database, ids)
}

async fn board(tracker: &Tracker<MemoryStore>) -> Board {
    let peer = tracker.peer();
    let (projects, _) = database(
        peer,
        "Projects",
        "PR",
        &[("Name", DatabaseFieldType::String, &[])],
    )
    .await;
    tracker
        .create(projects, "Launch", &[], Body::Keep)
        .await
        .unwrap();
    let (issues, fields) = database(
        peer,
        "Issues",
        "BE",
        &[
            ("Title", DatabaseFieldType::String, &[]),
            ("Status", DatabaseFieldType::Enum, &["To do", "Done"]),
            ("Project", DatabaseFieldType::Relation, &[]),
        ],
    )
    .await;
    let schema = peer
        .open::<DatabaseContent>(issues)
        .await
        .unwrap()
        .unwrap()
        .root()
        .schema
        .unwrap();
    let mut content = peer
        .open::<DatabaseSchemaContent>(schema)
        .await
        .unwrap()
        .unwrap();
    let head = peer.local_head(schema);
    content.apply(&DatabaseSchema::set_relation_options(
        fields[2],
        DatabaseRelationOptions {
            database: Some(projects),
        },
    ));
    peer.save(schema, &content, head).await.unwrap();
    Board {
        database: issues,
        status: fields[1],
        project: fields[2],
    }
}

fn invocation(command: Command) -> Invocation {
    Invocation {
        command,
        json: false,
    }
}
