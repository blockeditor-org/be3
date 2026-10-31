use std::{
    fmt::Display,
    fmt::Write as _,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use workspace_ui_types::{FolderContent, WorkspaceContent};

use be_block::{
    Allowance, BlockContent, BlockMetadata, MY_FILES, OrganizationPolicies,
    OrganizationPoliciesContent, Root, Settings, SettingsContent,
};
use be_cli::{cli, tracker::Tracker};
use be_client::{Credentials, Peer, PeerConfig};
use be_frozen::scene;
use be_graph::{Access, BlockParent};
use be_keys::{InviteCode, RecoveryPhrase};
use be_protocol::{OrganizationKind, OrganizationRole, client};
use be_server::{
    ServerConfig,
    backup::{self, BackupKey, Directory},
};
use be_store::{ChunkerConfig, MemoryStore};
use text_block_types::{TextBlock, TextContent, TextIndentation, TextLanguage};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

const CLIENT: u64 = 0x6672_6f7a_656e;

struct Written {
    organization: Uuid,
    personal: Uuid,
    owner_token: String,
    owner_phrase: RecoveryPhrase,
    editor_phrase: RecoveryPhrase,
    friend_token: String,
    friend_phrase: RecoveryPhrase,
    agent_token: String,
    agent_phrase: RecoveryPhrase,
    link_code: InviteCode,
    invite_code: InviteCode,
    blocks: Vec<(&'static str, Uuid)>,
}

fn main() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let (Some(root), Some(name)) = (arguments.next(), arguments.next()) else {
        return Err("usage: freeze SNAPSHOTS-DIRECTORY NAME, such as 2026_10_16".to_owned());
    };
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(format!(
            "{name:?} is not a snapshot name; use lowercase letters, digits and _, such as 2026_10_12"
        ));
    }
    let root = PathBuf::from(root);
    let archive_path = root.join(format!("{name}.zip"));
    let module = root.join(format!("s{name}.rs"));
    if archive_path.exists() || module.exists() {
        return Err(format!(
            "{} already exists, and a snapshot is never replaced",
            archive_path.display()
        ));
    }
    build_id::install("freeze");
    let data = std::env::temp_dir().join(format!("be-freeze-{}", Uuid::new_v4()));
    let target = data.with_extension("backup");
    let frozen = freeze(&name, &data, &target, &archive_path, &module);
    let _ = fs::remove_dir_all(&data);
    let _ = fs::remove_dir_all(&target);
    if frozen.is_err() {
        let _ = fs::remove_file(&archive_path);
        let _ = fs::remove_file(&module);
    }
    frozen?;
    println!(
        "wrote {} and {}; add `mod s{name};` and `&s{name}::SNAPSHOT` to SNAPSHOTS in {}",
        archive_path.display(),
        module.display(),
        root.with_extension("rs").display()
    );
    Ok(())
}

fn freeze(
    name: &str,
    data: &Path,
    target: &Path,
    archive_path: &Path,
    module: &Path,
) -> Result<(), String> {
    let written = tokio::runtime::Runtime::new()
        .map_err(text)?
        .block_on(serve_scene(data))?;
    settle(&data.join("metadata.sqlite"))?;

    let backup_key = BackupKey::generate();
    let report = backup::backup(
        data,
        &Directory(target.to_path_buf()),
        &backup_key,
        SystemTime::now(),
    )
    .map_err(text)?;
    for entry in fs::read_dir(data).map_err(text)? {
        let path = entry.map_err(text)?.path();
        if path.extension().is_some_and(|extension| extension == "txt") {
            fs::remove_file(path).map_err(text)?;
        }
    }

    let mut archive = zip::ZipWriter::new(fs::File::create(archive_path).map_err(text)?);
    pack(&mut archive, data, "data")?;
    let snapshot = format!("backup/databases/{}", report.snapshot);
    add(
        &mut archive,
        &snapshot,
        &target.join(&snapshot["backup/".len()..]),
    )?;
    archive.finish().map_err(text)?;
    fs::write(module, source(name, &written, &backup_key)).map_err(text)
}

async fn serve_scene(data: &Path) -> Result<Written, String> {
    be_server::ServerStore::open(data).map_err(text)?;
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(text)?;
    let url = format!("http://{}", listener.local_addr().map_err(text)?);
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(be_server::serve_with_config(
        listener,
        data.to_path_buf(),
        ServerConfig::OPEN,
        stopped,
    ));
    let written = write_scene(&url, data).await;
    let _ = stop.send(());
    let _ = tokio::time::timeout(Duration::from_secs(10), server).await;
    written
}

async fn register(
    data: &Path,
    socket: &str,
    email: &str,
    name: &str,
    password: &str,
    phrase: &RecoveryPhrase,
    organization: Option<Uuid>,
) -> Result<Peer<MemoryStore>, String> {
    Peer::connect(
        PeerConfig::new(
            socket,
            phrase.secret(),
            Credentials::Token(
                be_server::ServerStore::open(data)
                    .map_err(text)?
                    .register(email, name, password)
                    .await
                    .map_err(text)?
                    .1,
            ),
        )
        .organization(organization),
        MemoryStore::new(),
    )
    .await
    .map_err(text)
}

async fn reopen(
    socket: &str,
    peer: &Peer<MemoryStore>,
    organization: Uuid,
    chunker: ChunkerConfig,
) -> Result<Peer<MemoryStore>, String> {
    Peer::connect(
        PeerConfig::new(
            socket,
            peer.keys().identity().clone(),
            Credentials::Token(peer.token().to_owned()),
        )
        .organization(Some(organization))
        .chunker(chunker),
        MemoryStore::new(),
    )
    .await
    .map_err(text)
}

async fn text_block(peer: &Peer<MemoryStore>, parent: Uuid, body: &str) -> Result<Uuid, String> {
    let block = peer
        .create::<TextContent>(BlockParent::Block(parent))
        .await
        .map_err(text)?;
    peer.save(block, &TextBlock::of(body), None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;
    Ok(block)
}

async fn folder(
    peer: &Peer<MemoryStore>,
    parent: BlockParent,
    name: &str,
    content_type: Uuid,
    metadata: Option<BlockMetadata>,
) -> Result<Uuid, String> {
    Ok(peer
        .create_block(
            Uuid::new_v4(),
            content_type,
            parent,
            &metadata.unwrap_or_else(|| BlockMetadata::named(name)),
        )
        .await
        .map_err(text)?
        .id)
}

async fn fill(peer: &Peer<MemoryStore>, block: Uuid, children: &[Uuid]) -> Result<(), String> {
    let mut content = FolderContent::default();
    for child in children {
        let add = content.root().add(*child);
        content.apply(&add);
    }
    peer.save(block, &content, None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;
    Ok(())
}

async fn fill_workspace(
    peer: &Peer<MemoryStore>,
    block: Uuid,
    children: &[Uuid],
) -> Result<(), String> {
    let head = peer.summary(block).await.map_err(text)?.head;
    let mut content: WorkspaceContent = peer.open(block).await.map_err(text)?.unwrap_or_default();
    for child in children {
        let add = content.root().add(*child);
        content.apply(&add);
    }
    peer.save(block, &content, head)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;
    Ok(())
}

async fn set_policies(owner: &Peer<MemoryStore>) -> Result<(), String> {
    let settings = owner.organization_settings();
    let block = owner
        .create::<OrganizationPoliciesContent>(BlockParent::Block(settings))
        .await
        .map_err(text)?;
    let mut policies = OrganizationPoliciesContent::default();
    policies.apply(&OrganizationPolicies::set_outside_sharing(Some(
        Allowance::Blocked,
    )));
    owner
        .save(block, &policies, None)
        .await
        .map_err(text)?
        .published()
        .ok_or("the policies were not saved")?;
    let mut content = SettingsContent::default();
    content.apply(&Settings::set_policy(
        OrganizationPolicies::CONTENT_TYPE,
        block,
    ));
    let head = owner.summary(settings).await.map_err(text)?.head;
    owner
        .save(settings, &content, head)
        .await
        .map_err(text)?
        .published()
        .ok_or("the organization's settings were not saved")?;
    Ok(())
}

fn card(peer: &Peer<MemoryStore>, name: &str, email: &str) -> be_keys::ContactCard {
    be_client::contacts::card(peer.account(), peer.keys().identity(), name, email)
}

async fn write_scene(url: &str, data: &Path) -> Result<Written, String> {
    let socket = be_client::socket_url(url);
    let owner_phrase = RecoveryPhrase::generate();
    let owner = Peer::connect(
        PeerConfig {
            organization_name: scene::ORGANIZATION.into(),
            ..PeerConfig::new(
                &socket,
                owner_phrase.secret(),
                Credentials::Token(
                    be_server::ServerStore::open(data)
                        .map_err(text)?
                        .register(scene::OWNER_EMAIL, scene::OWNER_NAME, scene::OWNER_PASSWORD)
                        .await
                        .map_err(text)?
                        .1,
                ),
            )
        },
        MemoryStore::new(),
    )
    .await
    .map_err(text)?;
    let organization = owner.organization();
    let owner_token = owner.token().to_owned();
    let organizations = ask(client::organizations::list_organizations(
        &owner.connection(),
    ))
    .await?;
    let personal = organizations
        .iter()
        .find(|listed| listed.kind == OrganizationKind::Personal)
        .ok_or("registering joined no personal organization")?
        .id;
    let owner_card = card(&owner, scene::OWNER_NAME, scene::OWNER_EMAIL);

    let editor_phrase = RecoveryPhrase::generate();
    let editor = register(
        data,
        &socket,
        scene::EDITOR_EMAIL,
        scene::EDITOR_NAME,
        scene::EDITOR_PASSWORD,
        &editor_phrase,
        Some(personal),
    )
    .await?;
    let code = be_client::contacts::invite(
        &owner.connection(),
        owner.keys().identity(),
        &owner_card,
        organization,
        scene::EDITOR_EMAIL.into(),
        OrganizationRole::Administrator,
        owner.keys().escrow().as_ref(),
    )
    .await
    .map_err(text)?;
    be_client::contacts::accept_invitation(
        &editor.connection(),
        editor.keys().identity(),
        &card(&editor, scene::EDITOR_NAME, scene::EDITOR_EMAIL),
        &code,
    )
    .await
    .map_err(text)?;
    be_client::contacts::finish_links(&owner.connection(), owner.keys().identity(), organization)
        .await
        .map_err(text)?;
    owner.refresh_contacts().await.map_err(text)?;
    let editor = reopen(&socket, &editor, organization, ChunkerConfig::DEFAULT).await?;
    owner.tend().await.map_err(text)?;
    let editor_public = owner
        .keys()
        .contact(editor.account())
        .ok_or("the invitation did not make the editor a contact")?;

    let invite_code = be_client::contacts::invite(
        &owner.connection(),
        owner.keys().identity(),
        &owner_card,
        organization,
        scene::INVITED_EMAIL.into(),
        OrganizationRole::Editor,
        owner.keys().escrow().as_ref(),
    )
    .await
    .map_err(text)?;

    let agent_phrase = RecoveryPhrase::generate();
    let agent_public = agent_phrase.secret().public();
    let be_protocol::CreatedAgent {
        agent,
        token: agent_token,
    } = ask(client::members::create_agent(
        &owner.connection(),
        organization,
        scene::AGENT_NAME.into(),
        agent_public.0,
        owner_card.seal_own(&agent_phrase.secret()),
        owner.keys().escrow().map_or_else(Vec::new, |escrow| {
            be_client::contacts::seal_pin(&agent_phrase.secret(), organization, &escrow)
        }),
    ))
    .await?;
    let agent_card = be_keys::ContactCard {
        account: agent.account.into_bytes(),
        public: agent_public,
        display_name: agent.name.clone(),
        email: agent.email.clone(),
    };
    ask(client::contacts::put_contact(
        &owner.connection(),
        agent.account,
        agent_card.seal_own(owner.keys().identity()),
    ))
    .await?;
    owner.refresh_contacts().await.map_err(text)?;

    let mut mine = BlockMetadata::named(scene::MY_FILES);
    mine.insert(&MY_FILES, &true);
    let my_files = folder(
        &owner,
        BlockParent::Root,
        scene::MY_FILES,
        WorkspaceContent::CONTENT_TYPE,
        Some(mine),
    )
    .await?;
    let team = folder(
        &owner,
        BlockParent::Root,
        scene::TEAM,
        WorkspaceContent::CONTENT_TYPE,
        None,
    )
    .await?;
    let notes = folder(
        &owner,
        BlockParent::Block(my_files),
        scene::NOTES,
        FolderContent::CONTENT_TYPE,
        None,
    )
    .await?;
    let shopping = owner
        .create::<TextContent>(BlockParent::Block(notes))
        .await
        .map_err(text)?;
    let mut content = TextBlock::of(scene::SHOPPING[0]);
    let mut head = owner
        .save(shopping, &content, None)
        .await
        .map_err(text)?
        .published();
    for (index, pair) in scene::SHOPPING.windows(2).enumerate() {
        let added = pair[1]
            .strip_prefix(pair[0])
            .ok_or("each list extends the last")?;
        let edit = TextBlock::insert(&content, CLIENT, pair[0].len(), added.as_bytes())
            .ok_or("the text took no insert")?;
        content.apply(&edit);
        let saved = match index {
            0 => owner.save(shopping, &content, head).await,
            _ => owner.bookmark(shopping, &content, head, "").await,
        };
        head = saved.map_err(text)?.published();
    }
    head.ok_or("a save was rejected")?;

    let program = owner
        .create::<TextContent>(BlockParent::Block(notes))
        .await
        .map_err(text)?;
    let mut content = TextBlock::of(scene::PROGRAM);
    content.apply(&TextBlock::set_language(TextLanguage::Rust));
    content.apply(&TextBlock::set_indentation(TextIndentation::Tabs));
    owner
        .save(program, &content, None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;

    let archive = folder(
        &owner,
        BlockParent::Block(notes),
        scene::ARCHIVE,
        FolderContent::CONTENT_TYPE,
        None,
    )
    .await?;
    let chunked = reopen(&socket, &owner, organization, ChunkerConfig::SMALL).await?;
    let long = text_block(&chunked, archive, &scene::long_text()).await?;
    fill(&owner, archive, &[long]).await?;
    fill(&owner, notes, &[shopping, program, archive]).await?;
    fill_workspace(&owner, my_files, &[notes]).await?;

    let tracker = Tracker::new(
        reopen(&socket, &owner, organization, ChunkerConfig::DEFAULT).await?,
        sources(),
    );
    let (view, database) = tracker
        .create_database(scene::DATABASE, scene::PREFIX, BlockParent::Block(team))
        .await?;
    let statuses = scene::STATUSES.join(",");
    let shopping_id = shopping.to_string();
    let issues = scene::DATABASE;
    run(
        &tracker,
        &[
            issues,
            "add-column",
            "Status",
            "enum",
            "--options",
            &statuses,
        ],
    )
    .await?;
    run(
        &tracker,
        &[
            issues,
            "option",
            "Status",
            "Done",
            "closed=yes",
            "icon=done",
            "color=blue",
        ],
    )
    .await?;
    run(
        &tracker,
        &[issues, "option", "Status", "To do", "default=yes"],
    )
    .await?;
    for (column, kind) in [
        ("Estimate", "number"),
        ("Urgent", "boolean"),
        ("Tint", "color"),
        ("Due", "date"),
        ("Picture", "block"),
    ] {
        run(&tracker, &[issues, "add-column", column, kind]).await?;
    }
    run(
        &tracker,
        &[
            issues,
            "add-column",
            "Blocked by",
            "relation",
            "--links",
            issues,
            "--multiple",
            "--inverse",
            "Blocking",
        ],
    )
    .await?;
    let first = run(
        &tracker,
        &[
            issues,
            "new",
            scene::FIRST_ROW,
            "Status=Done",
            "Estimate=3",
            "Urgent=true",
            "Tint=#ff8800ff",
            "Due=2026-10-12 09:30",
            &format!("Picture={shopping_id}"),
            "--body",
            scene::FIRST_PAGE,
        ],
    )
    .await?;
    let first = first
        .split_whitespace()
        .next()
        .ok_or("the new row printed no id")?
        .to_owned();
    run(
        &tracker,
        &[
            issues,
            "new",
            scene::SECOND_ROW,
            "Status=In progress",
            &format!("Blocked by={first}"),
        ],
    )
    .await?;
    owner
        .share(team, agent.account, Access::Edit, &agent_public)
        .await
        .map_err(text)?;

    owner
        .set_role(editor.account(), OrganizationRole::Editor)
        .await
        .map_err(text)?;
    owner.tend().await.map_err(text)?;

    let plan = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .map_err(text)?;
    let before = owner
        .save(plan, &TextBlock::of(scene::PLAN[0]), None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;
    fill_workspace(&owner, team, &[view, plan]).await?;
    owner
        .share(team, editor.account(), Access::Edit, &editor_public)
        .await
        .map_err(text)?;
    owner
        .grant(team, editor.account(), Access::None)
        .await
        .map_err(text)?;
    owner.tend().await.map_err(text)?;
    owner
        .save(plan, &TextBlock::of(scene::PLAN[1]), Some(before))
        .await
        .map_err(text)?
        .published()
        .ok_or("a save after the rotation was rejected")?;
    set_policies(&owner).await?;

    let owner_personal = reopen(&socket, &owner, personal, ChunkerConfig::DEFAULT).await?;
    let friend_phrase = RecoveryPhrase::generate();
    let friend = register(
        data,
        &socket,
        scene::FRIEND_EMAIL,
        scene::FRIEND_NAME,
        scene::FRIEND_PASSWORD,
        &friend_phrase,
        Some(personal),
    )
    .await?;
    let friend_token = friend.token().to_owned();
    let link = be_client::contacts::make_link(
        &owner_personal.connection(),
        owner_personal.keys().identity(),
        &owner_card,
        None,
    )
    .await
    .map_err(text)?;
    be_client::contacts::accept_link(
        &friend.connection(),
        friend.keys().identity(),
        &card(&friend, scene::FRIEND_NAME, scene::FRIEND_EMAIL),
        &link,
    )
    .await
    .map_err(text)?;
    be_client::contacts::finish_links(
        &owner_personal.connection(),
        owner_personal.keys().identity(),
        personal,
    )
    .await
    .map_err(text)?;
    owner_personal.refresh_contacts().await.map_err(text)?;
    let friend_public = owner_personal
        .keys()
        .contact(friend.account())
        .ok_or("the link did not make the friend a contact")?;

    let mut mine = BlockMetadata::named(scene::MY_FILES);
    mine.insert(&MY_FILES, &true);
    let personal_files = folder(
        &owner_personal,
        BlockParent::Root,
        scene::MY_FILES,
        WorkspaceContent::CONTENT_TYPE,
        Some(mine),
    )
    .await?;
    let holiday = folder(
        &owner_personal,
        BlockParent::Block(personal_files),
        scene::HOLIDAY,
        FolderContent::CONTENT_TYPE,
        None,
    )
    .await?;
    let holiday_note = text_block(&owner_personal, holiday, scene::HOLIDAY_NOTE).await?;
    fill(&owner_personal, holiday, &[holiday_note]).await?;
    let recipes = text_block(&owner_personal, personal_files, scene::RECIPES).await?;
    fill_workspace(&owner_personal, personal_files, &[holiday, recipes]).await?;
    owner_personal
        .share(holiday, friend.account(), Access::Edit, &friend_public)
        .await
        .map_err(text)?;
    let offer = owner_personal
        .offer(recipes, Access::View)
        .await
        .map_err(text)?;
    let link_code = be_client::contacts::make_link(
        &owner_personal.connection(),
        owner_personal.keys().identity(),
        &owner_card,
        Some(&offer),
    )
    .await
    .map_err(text)?;

    let fresh = RecoveryPhrase::generate();
    let be_protocol::RecoveryKey { sealed, own, .. } =
        ask(client::keys::get_recovery_key(&friend.connection())).await?;
    let resealed = be_client::contacts::new_identity(
        friend.account(),
        friend.keys().identity(),
        &fresh.secret(),
        sealed,
        own,
    )
    .ok_or("the friend's old phrase did not open what was sealed to it")?;
    ask(client::keys::set_recovery_key(
        &friend.connection(),
        fresh.secret().public().0,
        resealed.sealed,
        resealed.own,
        resealed.changes,
    ))
    .await?;

    Ok(Written {
        organization,
        personal,
        owner_token,
        owner_phrase,
        editor_phrase,
        friend_token,
        friend_phrase: fresh,
        agent_token,
        agent_phrase,
        link_code,
        invite_code,
        blocks: vec![
            ("my_files", my_files),
            ("team", team),
            ("notes", notes),
            ("shopping", shopping),
            ("program", program),
            ("archive", archive),
            ("long", long),
            ("view", view),
            ("database", database),
            ("plan", plan),
            ("personal_files", personal_files),
            ("holiday", holiday),
            ("holiday_note", holiday_note),
            ("recipes", recipes),
        ],
    })
}

async fn ask<T: be_ops::DeserializeOwned>(
    pending: be_ops::net::Pending<Result<T, be_protocol::Error>>,
) -> Result<T, String> {
    be_client::ask(pending).await.map_err(text)
}

async fn run(tracker: &Tracker<MemoryStore>, arguments: &[&str]) -> Result<String, String> {
    let invocation = cli::parse(
        arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect(),
        || Err("no standard input".to_owned()),
    )?;
    match cli::run(tracker, &invocation).await? {
        cli::Outcome::Print(printed) => Ok(printed),
        cli::Outcome::Edit { .. } | cli::Outcome::Watch { .. } => {
            Err(format!("{arguments:?} asked for an editor or a watch"))
        }
    }
}

fn settle(database: &Path) -> Result<(), String> {
    let connection = rusqlite::Connection::open(database).map_err(text)?;
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode = DELETE; VACUUM;")
        .map_err(text)?;
    drop(connection);
    for suffix in ["-wal", "-shm"] {
        let mut path = database.as_os_str().to_owned();
        path.push(suffix);
        let _ = fs::remove_file(PathBuf::from(path));
    }
    Ok(())
}

fn pack(archive: &mut zip::ZipWriter<fs::File>, from: &Path, prefix: &str) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(from)
        .map_err(text)?
        .collect::<Result<_, _>>()
        .map_err(text)?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        if entry.file_type().map_err(text)?.is_dir() {
            pack(archive, &entry.path(), &name)?;
        } else {
            add(archive, &name, &entry.path())?;
        }
    }
    Ok(())
}

fn add(archive: &mut zip::ZipWriter<fs::File>, name: &str, from: &Path) -> Result<(), String> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default());
    archive.start_file(name, options).map_err(text)?;
    archive
        .write_all(&fs::read(from).map_err(text)?)
        .map_err(text)
}

fn source(name: &str, written: &Written, backup_key: &BackupKey) -> String {
    let mut out = format!(
        "use super::Snapshot;\n\npub const SNAPSHOT: Snapshot = Snapshot {{\n    name: \"{name}\",\n"
    );
    let mut field = |label: &str, value: &dyn Display| {
        let _ = writeln!(out, "    {label}: \"{value}\",");
    };
    field("organization", &written.organization);
    field("personal", &written.personal);
    field("owner_token", &written.owner_token);
    field("owner_phrase", &written.owner_phrase);
    field("editor_phrase", &written.editor_phrase);
    field("friend_token", &written.friend_token);
    field("friend_phrase", &written.friend_phrase);
    field("agent_token", &written.agent_token);
    field("agent_phrase", &written.agent_phrase);
    field("link_code", &written.link_code);
    field("invite_code", &written.invite_code);
    field("backup_key", &backup_key.to_hex());
    for (label, block) in &written.blocks {
        field(label, block);
    }
    let _ = writeln!(out, "    files: include_bytes!(\"{name}.zip\"),\n}};");
    out
}

fn text(error: impl Display) -> String {
    error.to_string()
}

fn sources() -> be_cli::tracker::Sources {
    Box::new(first_party_types::sources)
}
