use std::{
    fmt::Display,
    fmt::Write as _,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use be_block::{
    BlockContent, BlockMetadata, FolderContent, TextBlock, TextContent, TextIndentation,
    TextLanguage,
};
use be_cli::{cli, tracker::Tracker};
use be_client::{Connection, Credentials, Peer, PeerConfig};
use be_frozen::scene;
use be_graph::{Access, BlockParent};
use be_keys::{InviteCode, RecoveryPhrase};
use be_protocol::{ClientMessage, SealedKey, ServerMessage, WorkspaceRole};
use be_server::{
    ServerConfig,
    backup::{self, BackupKey, Directory},
};
use be_store::{ChunkerConfig, ContentKey, MemoryStore};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

const CLIENT: u64 = 0x6672_6f7a_656e;

struct Written {
    workspace: Uuid,
    key: ContentKey,
    owner_token: String,
    owner_phrase: RecoveryPhrase,
    editor_phrase: RecoveryPhrase,
    agent_token: String,
    agent_phrase: RecoveryPhrase,
    blocks: Vec<(&'static str, Uuid)>,
}

fn main() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let (Some(root), Some(name)) = (arguments.next(), arguments.next()) else {
        return Err("usage: freeze SNAPSHOTS-DIRECTORY NAME, such as 2026_10_12".to_owned());
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
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(text)?;
    let url = format!("http://{}", listener.local_addr().map_err(text)?);
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(be_server::serve_with_config(
        listener,
        data.to_path_buf(),
        ServerConfig::OPEN,
        stopped,
    ));
    let written = write_scene(&url).await;
    let _ = stop.send(());
    let _ = tokio::time::timeout(Duration::from_secs(10), server).await;
    written
}

async fn write_scene(url: &str) -> Result<Written, String> {
    let socket = be_client::socket_url(url);
    let key = ContentKey::random();
    let owner = Peer::connect(
        PeerConfig {
            workspace_name: scene::WORKSPACE.into(),
            ..PeerConfig::new(
                &socket,
                key,
                Credentials::Register {
                    email: scene::OWNER_EMAIL.into(),
                    display_name: scene::OWNER_NAME.into(),
                    password: scene::OWNER_PASSWORD.into(),
                },
            )
        },
        MemoryStore::new(),
    )
    .await
    .map_err(text)?;
    let workspace = owner.workspace();
    let owner_token = owner.token().to_owned();
    let signed_in = |chunker| {
        Peer::connect(
            PeerConfig::new(&socket, key, Credentials::Token(owner_token.clone()))
                .workspace(Some(workspace))
                .chunker(chunker),
            MemoryStore::new(),
        )
    };

    let owner_phrase = RecoveryPhrase::generate();
    let public = owner_phrase.secret().public();
    request(owner.connection(), |request| {
        ClientMessage::SetRecoveryKey {
            request,
            public: public.0,
            sealed: vec![SealedKey {
                workspace,
                sealed: public.seal(key.as_bytes()),
            }],
        }
    })
    .await?;

    let editor = Connection::connect(&socket).await.map_err(text)?;
    let ServerMessage::Authenticated {
        account: editor_account,
        ..
    } = request(&editor, |request| ClientMessage::Register {
        request,
        email: scene::EDITOR_EMAIL.into(),
        display_name: scene::EDITOR_NAME.into(),
        password: scene::EDITOR_PASSWORD.into(),
    })
    .await?
    else {
        return Err("the editor was not registered".to_owned());
    };
    request(owner.connection(), |request| ClientMessage::Invite {
        request,
        workspace,
        email: scene::EDITOR_EMAIL.into(),
        role: WorkspaceRole::Editor,
        sealed: InviteCode::generate().seal(workspace.as_bytes(), key.as_bytes()),
    })
    .await?;
    let ServerMessage::Invitations { invitations, .. } = request(&editor, |request| {
        ClientMessage::ListInvitations { request }
    })
    .await?
    else {
        return Err("the editor's invitations were not listed".to_owned());
    };
    let invitation = invitations.first().ok_or("the editor was not invited")?.id;
    request(&editor, |request| ClientMessage::RespondInvitation {
        request,
        invitation,
        accept: true,
    })
    .await?;
    let editor_phrase = RecoveryPhrase::generate();
    let editor_public = editor_phrase.secret().public();
    request(&editor, |request| ClientMessage::SetRecoveryKey {
        request,
        public: editor_public.0,
        sealed: Vec::new(),
    })
    .await?;
    request(&editor, |request| ClientMessage::PutWorkspaceKey {
        request,
        workspace,
        sealed: editor_public.seal(key.as_bytes()),
    })
    .await?;
    request(owner.connection(), |request| ClientMessage::Invite {
        request,
        workspace,
        email: scene::INVITED_EMAIL.into(),
        role: WorkspaceRole::Editor,
        sealed: InviteCode::generate().seal(workspace.as_bytes(), key.as_bytes()),
    })
    .await?;

    let agent_phrase = RecoveryPhrase::generate();
    let agent_public = agent_phrase.secret().public();
    let ServerMessage::AgentCreated {
        agent,
        token: agent_token,
        ..
    } = request(owner.connection(), |request| ClientMessage::CreateAgent {
        request,
        workspace,
        name: scene::AGENT_NAME.into(),
        recovery: agent_public.0,
        sealed: agent_public.seal(key.as_bytes()),
    })
    .await?
    else {
        return Err("the agent was not created".to_owned());
    };

    let notes = owner
        .create_block(
            Uuid::new_v4(),
            FolderContent::CONTENT_TYPE,
            BlockParent::Root,
            &BlockMetadata::named(scene::NOTES),
        )
        .await
        .map_err(text)?
        .id;
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
    owner
        .grant(shopping, editor_account, Access::View)
        .await
        .map_err(text)?;

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

    let archive = owner
        .create_block(
            Uuid::new_v4(),
            FolderContent::CONTENT_TYPE,
            BlockParent::Block(notes),
            &BlockMetadata::named(scene::ARCHIVE),
        )
        .await
        .map_err(text)?
        .id;
    let chunked = signed_in(ChunkerConfig::SMALL).await.map_err(text)?;
    let long = chunked
        .create::<TextContent>(BlockParent::Block(archive))
        .await
        .map_err(text)?;
    chunked
        .save(long, &TextBlock::of(&scene::long_text()), None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;
    let mut folder = FolderContent::default();
    let add = folder.root().add(long);
    folder.apply(&add);
    owner
        .save(archive, &folder, None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;

    let tracker = Tracker::new(signed_in(ChunkerConfig::DEFAULT).await.map_err(text)?);
    let (view, database) = tracker
        .create_database(scene::DATABASE, scene::PREFIX, BlockParent::Block(notes))
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
        .grant(notes, agent.account, Access::Edit)
        .await
        .map_err(text)?;

    let mut folder = FolderContent::default();
    for block in [shopping, program, archive, view] {
        let add = folder.root().add(block);
        folder.apply(&add);
    }
    owner
        .save(notes, &folder, None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;

    Ok(Written {
        workspace,
        key,
        owner_token,
        owner_phrase,
        editor_phrase,
        agent_token,
        agent_phrase,
        blocks: vec![
            ("notes", notes),
            ("shopping", shopping),
            ("program", program),
            ("archive", archive),
            ("long", long),
            ("view", view),
            ("database", database),
        ],
    })
}

async fn request(
    connection: &Connection,
    build: impl FnOnce(u64) -> ClientMessage,
) -> Result<ServerMessage, String> {
    connection.request(build).await.map_err(text)
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
        cli::Outcome::Edit { .. } => Err(format!("{arguments:?} asked for an editor")),
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
    field("workspace", &written.workspace);
    field("workspace_key", &hex(written.key.as_bytes()));
    field("owner_token", &written.owner_token);
    field("owner_phrase", &written.owner_phrase);
    field("editor_phrase", &written.editor_phrase);
    field("agent_token", &written.agent_token);
    field("agent_phrase", &written.agent_phrase);
    field("backup_key", &backup_key.to_hex());
    for (label, block) in &written.blocks {
        field(label, block);
    }
    let _ = writeln!(out, "    files: include_bytes!(\"{name}.zip\"),\n}};");
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn text(error: impl Display) -> String {
    error.to_string()
}
