use std::path::PathBuf;

use be_block_client::{Credentials, Peer, PeerConfig};
use be_block_content::{BlockContent, BlockMetadata};
use be_block_graph::BlockParent;
use be_block_store::MemoryStore;
use be_server::Running;
use be_type_database::database::{Database, DatabaseContent};
use be_type_database::database_schema::{
    DatabaseFieldType, DatabaseRelationOptions, DatabaseSchema, DatabaseSchemaContent,
};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::blocks::Entry;
use crate::cli::{self, Invocation, Outcome};
use crate::session::Settings;
use crate::tracker::{Assignment, Body, RowView, Tracker};

mod a_block_only_its_owner_reads_is_not_downloaded;
mod a_choice_option_takes_a_colour_an_icon_and_closes_rows;
mod a_csv_export_is_imported_as_rows_with_pages;
mod a_database_in_a_view_only_its_owner_reads_is_not_downloaded;
mod a_database_moved_to_another_workspace_takes_its_prefix_along;
mod a_download_that_cannot_check_the_blocks_above_is_refused;
mod a_downloaded_block_uploads_as_a_copy_with_new_ids;
mod a_downloaded_chat_keeps_its_signed_log;
mod a_folder_syncs_with_a_directory;
mod a_manifest_says_which_of_its_types_count_what_is_unread;
mod a_manifest_says_which_of_its_types_have_a_plain_file;
mod a_merged_pull_request_closes_rows_without_a_done_status;
mod a_new_row_takes_each_choice_fields_default;
mod a_page_saved_elsewhere_keeps_both_changes;
mod a_pull_request_moves_the_rows_it_names;
mod a_reader_sees_edits_a_session_journaled_before_they_are_committed;
mod a_restarted_terminal_host_keeps_the_history_and_marks_the_restore;
mod a_row_blocked_by_another_shows_as_blocking_there;
mod a_row_id_resolves_through_the_workspaces_prefixes;
mod a_spreadsheet_escaped_cell_is_read_as_written;
mod a_terminal_host_encodes_keys_for_the_mode_its_program_set;
mod a_terminal_host_keeps_its_standing_in_its_status_file;
mod a_terminal_host_runs_only_on_the_sandbox_it_was_installed_for;
mod a_terminal_host_writes_its_shell_into_the_document_for_its_viewers;
mod a_token_is_never_sent_unencrypted_to_another_machine;
mod agent_credentials_are_read_as_one_json_bundle;
mod an_agent_reads_and_writes_the_rows_shared_with_it;
mod an_edited_page_keeps_what_someone_else_changed_meanwhile;
mod an_import_links_rows_to_each_other_by_their_old_ids;
mod arguments_parse_into_commands;
mod be_hosts_a_block_while_no_other_device_does;
mod be_reads_a_block_type_from_the_type_module_staged_beside_it;
mod be_runs_an_apps_host_component_with_no_display;
mod be_signs_in_with_only_an_injected_authorization_header;
mod blocks_are_listed_and_found_by_short_id_or_name;
mod malformed_agent_credentials_do_not_disclose_secrets;
mod proxy_credentials_do_not_need_an_account_token;
mod rows_are_added_filtered_changed_and_given_pages;
mod terminal;
mod two_terminal_hosts_take_turns_and_the_second_stands_by;

/// Every editor's manifest in place of the apps discovery found, for the whole process, as a
/// `be` with every app beside it would find them: the tests run the types in process
/// (`be_type_bundle`), and a module discovery opens is still read from BE_BUNDLE.
fn manifests() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let editors = std::env::var_os("EDITOR_MANIFESTS")
            .map(PathBuf::from)
            .expect("EDITOR_MANIFESTS names the editors' manifests");
        let mut documents = Vec::new();
        for entry in std::fs::read_dir(&editors).expect("EDITOR_MANIFESTS is a directory") {
            let path = entry
                .expect("the entry is readable")
                .path()
                .join("manifest.json");
            let document = std::fs::read_to_string(&path).expect("the manifest is readable");
            documents.push((path.display().to_string(), document));
        }
        let documents: Vec<(&str, &str)> = documents
            .iter()
            .map(|(source, document)| (source.as_str(), document.as_str()))
            .collect();
        std::mem::forget(be_host_stack::discovery::install_for_testing(&documents));
    });
}

fn sources() -> crate::tracker::Sources {
    manifests();
    Box::new(be_type_bundle::sources)
}

struct Harness(Running);

impl std::ops::Deref for Harness {
    type Target = Running;

    fn deref(&self) -> &Running {
        &self.0
    }
}

impl Harness {
    async fn start() -> Self {
        be_lib_build_id::install("test");
        manifests();
        Self(Running::start(be_server::ServerConfig::OPEN).await)
    }

    async fn owner(&self) -> Peer<MemoryStore> {
        let owner = Peer::connect(
            PeerConfig::new(
                be_block_client::socket_url(&self.url),
                be_block_keys::RecoveryPhrase::generate().secret(),
                Credentials::Token(self.register("owner@example.com", "Owner").await),
            ),
            MemoryStore::new(),
        )
        .await
        .unwrap();
        workspace(&owner, "Tracker").await;
        owner
    }

    async fn stop(self) {
        self.0.stop().await;
    }
}

/// An agent of the owner's, bound to `bound_to` when it names a block (which grants it Edit
/// there), and the settings `be` signs in with as it.
async fn agent(
    harness: &Harness,
    owner: &Tracker<MemoryStore>,
    bound_to: Option<Uuid>,
) -> (Settings, Uuid, be_block_keys::RecoveryPublic) {
    let phrase = be_block_keys::RecoveryPhrase::generate();
    let public = phrase.secret().public();
    let peer = owner.peer();
    let organization = peer.organization();
    let account = Uuid::from_bytes(be_block_keys::agent_account(
        peer.account().into_bytes(),
        &phrase.secret().identity(),
    ));
    let bound = match bound_to {
        Some(block) => Some(be_block_protocol::BoundAgent {
            block,
            signed: peer
                .grant_record(
                    block,
                    account,
                    be_block_graph::Access::Edit,
                    phrase.secret().identity(),
                )
                .await
                .expect("signing the agent's grant failed"),
        }),
        None => None,
    };
    let be_block_protocol::CreatedAgent { agent, token } = be_block_client::ask(
        be_block_protocol::write::<be_block_protocol::CreatedAgent, _>(
            &peer.connection(),
            Some(organization),
            be_block_protocol::Op::CreateAgent {
                account: account,
                name: "Claude".into(),
                recovery: public.0,
                signing: phrase.secret().signing().0,
                contact: be_block_client::contacts::OwnContact::new(
                    be_block_client::contacts::card(
                        peer.account(),
                        peer.keys().identity(),
                        "Owner",
                        "owner@example.com",
                    ),
                    0,
                )
                .seal(&phrase.secret()),
                record: Vec::new(),
                escrow: be_block_client::contacts::seal_pin(
                    &phrase.secret(),
                    organization,
                    &peer.keys().pins(),
                ),
                bound_to: bound,
                member: peer
                    .agent_record(account, phrase.secret().identity())
                    .await
                    .expect("signing the agent in failed"),
            },
        ),
    )
    .await
    .expect("creating the agent failed");
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
        be_type_workspace::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &BlockMetadata::named(name),
        false,
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
        false,
    )
    .await
    .unwrap();
    peer.create_block(
        schema_id,
        DatabaseSchemaContent::CONTENT_TYPE,
        BlockParent::Block(database),
        &BlockMetadata::default(),
        false,
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

async fn run<S: be_block_store::ObjectStore>(
    tracker: &Tracker<S>,
    line: &str,
) -> Result<String, String> {
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
