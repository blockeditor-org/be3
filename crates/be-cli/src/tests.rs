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

use crate::cli::{self, Invocation, Outcome};
use crate::tracker::{Assignment, Body, RowView, Tracker};

mod a_choice_option_takes_a_colour_an_icon_and_closes_rows;
mod a_csv_export_is_imported_as_rows_with_pages;
mod a_deleted_database_frees_its_prefix;
mod a_downloaded_block_uploads_as_a_copy_with_new_ids;
mod a_merged_pull_request_closes_rows_without_a_done_status;
mod a_new_row_takes_each_choice_fields_default;
mod a_page_saved_elsewhere_keeps_both_changes;
mod a_pull_request_moves_the_rows_it_names;
mod a_row_blocked_by_another_shows_as_blocking_there;
mod a_spreadsheet_escaped_cell_is_read_as_written;
mod a_token_is_never_sent_unencrypted_to_another_machine;
mod an_agent_reads_and_writes_the_rows_shared_with_it;
mod an_edited_page_keeps_what_someone_else_changed_meanwhile;
mod an_import_links_rows_to_each_other_by_their_old_ids;
mod arguments_parse_into_commands;
mod blocks_are_listed_and_found_by_short_id_or_name;
mod rows_are_added_filtered_changed_and_given_pages;
mod two_databases_with_one_prefix_are_refused;

const KEY: [u8; 32] = [9; 32];

struct Harness {
    url: String,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Harness {
    async fn start() -> Self {
        build_id::install("test");
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
            ..Default::default()
        },
    ));
    peer.save(schema, &content, head).await.unwrap();
    tracker.invalidate().await;
    Board {
        database: issues,
        status: fields[1],
        project: fields[2],
    }
}

fn pairs(row: &RowView) -> Vec<(String, String)> {
    row.fields
        .iter()
        .map(|field| (field.name.clone(), field.text.clone()))
        .collect()
}

fn set(field: &str, value: &str) -> Assignment {
    Assignment::parse(&format!("{field}={value}")).unwrap()
}

async fn run(tracker: &Tracker<MemoryStore>, line: &str) -> Result<String, String> {
    let invocation: Invocation = cli::parse(
        line.split(' ').map(|word| word.replace('_', " ")).collect(),
        || Ok(String::new()),
    )?;
    match cli::run(tracker, &invocation).await? {
        Outcome::Print(text) => Ok(text),
        Outcome::Edit { .. } => Err("the command wanted an editor".to_owned()),
    }
}
