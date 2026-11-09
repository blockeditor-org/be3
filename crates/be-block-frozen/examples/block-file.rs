use be_type_workspace::FolderContent;
use std::{fmt::Display, path::PathBuf};

use be_block::{BlockContent, BlockMetadata};
use be_block_client::{Credentials, Peer, PeerConfig};
use be_block_frozen::scene;
use be_block_graph::BlockParent;
use be_block_store::MemoryStore;
use be_tool_cli::{cli, tracker::Tracker};
use be_type_text::{TextBlock, TextContent};
use uuid::Uuid;

fn main() -> Result<(), String> {
    let Some(file) = std::env::args().nth(1) else {
        return Err(
            "usage: block-file FILE, such as crates/be-block-frozen/src/fixtures/tracker.block"
                .to_owned(),
        );
    };
    let file = PathBuf::from(file);
    if file.exists() {
        return Err(format!(
            "{} already exists, and a fixture is never replaced",
            file.display()
        ));
    }
    be_lib_build_id::install("block-file");
    let bytes = tokio::runtime::Runtime::new()
        .map_err(text)?
        .block_on(write());
    std::fs::write(&file, bytes?).map_err(text)?;
    println!("wrote {}", file.display());
    Ok(())
}

async fn write() -> Result<Vec<u8>, String> {
    let server = be_server::Running::start(be_server::ServerConfig::OPEN).await;
    let owner = Peer::connect(
        PeerConfig::new(
            be_block_client::socket_url(&server.url),
            be_block_keys::RecoveryPhrase::generate().secret(),
            Credentials::Token(server.register(scene::OWNER_EMAIL, scene::OWNER_NAME).await),
        ),
        MemoryStore::new(),
    )
    .await
    .map_err(text)?;
    let notes = owner
        .create_block(
            Uuid::new_v4(),
            FolderContent::CONTENT_TYPE,
            BlockParent::Root,
            &BlockMetadata::named(scene::NOTES),
            false,
        )
        .await
        .map_err(text)?
        .id;
    let shopping = owner
        .create::<TextContent>(BlockParent::Block(notes))
        .await
        .map_err(text)?;
    owner
        .save(shopping, &TextBlock::of(scene::SHOPPING[2]), None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;

    let tracker = Tracker::new(owner, sources());
    let (view, _) = tracker
        .create_database(scene::DATABASE, scene::PREFIX, BlockParent::Block(notes))
        .await?;
    let issues = scene::DATABASE;
    let statuses = scene::STATUSES.join(",");
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
        &[issues, "option", "Status", "Done", "closed=yes"],
    )
    .await?;
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

    let mut folder = FolderContent::default();
    for block in [shopping, view] {
        let add = folder.root().add(block);
        folder.apply(&add);
    }
    tracker
        .peer()
        .save(notes, &folder, None)
        .await
        .map_err(text)?
        .published()
        .ok_or("a save was rejected")?;
    tracker.invalidate().await;
    let entry = tracker.entry(notes).await?;
    let (_, blocks) = tracker.download(&entry).await?;
    Ok(be_block_client::transfer::encode(&blocks))
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

fn text(error: impl Display) -> String {
    error.to_string()
}

fn sources() -> be_tool_cli::tracker::Sources {
    Box::new(be_type_bundle::sources)
}
