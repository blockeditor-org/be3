use std::path::PathBuf;

use be_block::{BlockContent, BlockMetadata};
use be_client::{Credentials, Peer, PeerConfig};
use be_graph::BlockParent;
use be_store::MemoryStore;
use database_types::database::{Database, DatabaseContent};
use database_types::database_schema::{
    DatabaseFieldType, DatabaseRelationOptions, DatabaseSchema, DatabaseSchemaContent,
};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

use crate::blocks::Entry;
use crate::cli::{self, Invocation, Outcome};
use crate::session::Settings;
use crate::tracker::{Assignment, Body, RowView, Tracker};

mod a_choice_option_takes_a_colour_an_icon_and_closes_rows;
mod a_csv_export_is_imported_as_rows_with_pages;
mod a_downloaded_block_uploads_as_a_copy_with_new_ids;
mod a_folder_syncs_with_a_directory;
mod a_merged_pull_request_closes_rows_without_a_done_status;
mod a_new_row_takes_each_choice_fields_default;
mod a_page_saved_elsewhere_keeps_both_changes;
mod a_pull_request_moves_the_rows_it_names;
mod a_restarted_terminal_host_keeps_the_history_and_marks_the_restore;
mod a_row_blocked_by_another_shows_as_blocking_there;
mod a_row_id_resolves_through_the_workspaces_prefixes;
mod a_spreadsheet_escaped_cell_is_read_as_written;
mod a_terminal_host_encodes_keys_for_the_mode_its_program_set;
mod a_terminal_host_writes_its_shell_into_the_document_for_its_viewers;
mod a_token_is_never_sent_unencrypted_to_another_machine;
mod agent_credentials_are_read_as_one_json_bundle;
mod an_agent_reads_and_writes_the_rows_shared_with_it;
mod an_edited_page_keeps_what_someone_else_changed_meanwhile;
mod an_import_links_rows_to_each_other_by_their_old_ids;
mod arguments_parse_into_commands;
mod blocks_are_listed_and_found_by_short_id_or_name;
mod malformed_agent_credentials_do_not_disclose_secrets;
mod proxy_credentials_do_not_need_an_account_token;
mod rows_are_added_filtered_changed_and_given_pages;
mod terminal;
mod the_plugin_bundle_holds_each_manifest_and_its_type_module;
mod two_terminal_hosts_take_turns_and_the_second_stands_by;

fn manifests() {
    if !crate::plugins::installed().is_empty() {
        return;
    }
    let editors = std::env::var_os("EDITOR_MANIFESTS")
        .map(PathBuf::from)
        .expect("EDITOR_MANIFESTS names the editors' manifests");
    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(&editors).expect("EDITOR_MANIFESTS is a directory") {
        let path = entry
            .expect("the entry is readable")
            .path()
            .join("manifest.json");
        let document = std::fs::read_to_string(&path).expect("the manifest is readable");
        plugins.push(crate::plugins::Plugin {
            manifest: block_plugin_api::manifest_from_json(&document)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display())),
            module: &[],
        });
    }
    crate::plugins::install(plugins);
}

fn sources() -> crate::tracker::Sources {
    manifests();
    Box::new(first_party_types::sources)
}

struct Harness {
    url: String,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Harness {
    async fn start() -> Self {
        build_id::install("test");
        manifests();
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
        let owner = Peer::connect(
            PeerConfig::new(
                be_client::socket_url(&self.url),
                be_keys::RecoveryPhrase::generate().secret(),
                Credentials::Token(
                    be_server::ServerStore::open(&self.directory)
                        .unwrap()
                        .register("owner@example.com", "Owner", "correct horse battery")
                        .await
                        .unwrap()
                        .1,
                ),
            ),
            MemoryStore::new(),
        )
        .await
        .unwrap();
        workspace(&owner, "Tracker").await;
        owner
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

async fn agent(
    harness: &Harness,
    owner: &Tracker<MemoryStore>,
) -> (Settings, Uuid, be_keys::RecoveryPublic) {
    let phrase = be_keys::RecoveryPhrase::generate();
    let public = phrase.secret().public();
    let peer = owner.peer();
    let organization = peer.organization();
    let response = peer
        .connection()
        .request(|request| be_protocol::ClientMessage::CreateAgent {
            request,
            organization,
            name: "Claude".into(),
            recovery: public.0,
            contact: be_client::contacts::card(
                peer.account(),
                peer.keys().identity(),
                "Owner",
                "owner@example.com",
            )
            .seal_own(&phrase.secret()),
            escrow: peer.keys().escrow().map_or_else(Vec::new, |escrow| {
                be_client::contacts::seal_pin(&phrase.secret(), organization, &escrow)
            }),
        })
        .await
        .unwrap();
    let be_protocol::ServerMessage::AgentCreated { agent, token, .. } = response else {
        panic!("creating the agent failed: {response:?}");
    };
    let credentials = serde_json::json!({
        "server": harness.url,
        "organization": organization,
        "token": token,
        "recovery_phrase": phrase.to_string(),
    })
    .to_string();
    let cache = std::env::temp_dir().join(format!("be-cli-cache-{}", Uuid::new_v4()));
    let settings = Settings::from_values(|name| match name {
        "BE3_AGENT_CREDENTIALS" => Some(credentials.clone()),
        "BE3_CACHE" => Some(cache.to_string_lossy().into_owned()),
        _ => None,
    })
    .unwrap();
    (settings, agent.account, public)
}

async fn workspace(peer: &Peer<MemoryStore>, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    peer.create_block(
        id,
        workspace_ui_types::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &BlockMetadata::named(name),
    )
    .await
    .unwrap();
    id
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
    let parent = peer
        .children(BlockParent::Root)
        .await
        .unwrap()
        .first()
        .map_or(BlockParent::Root, |workspace| {
            BlockParent::Block(workspace.id)
        });
    peer.create_block(
        database,
        DatabaseContent::CONTENT_TYPE,
        parent,
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

async fn run<S: be_store::ObjectStore>(tracker: &Tracker<S>, line: &str) -> Result<String, String> {
    let invocation: Invocation = cli::parse(
        line.split(' ').map(|word| word.replace('_', " ")).collect(),
        || Ok(String::new()),
    )?;
    match cli::run(tracker, &invocation).await? {
        Outcome::Print(text) => Ok(text),
        Outcome::Edit { .. } => Err("the command wanted an editor".to_owned()),
        Outcome::Watch { .. } => Err("the command wanted to keep watching".to_owned()),
    }
}
