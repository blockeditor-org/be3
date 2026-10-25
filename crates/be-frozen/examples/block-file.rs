use std::{fmt::Display, path::PathBuf, time::Duration};
use workspace_ui_types::FolderContent;

use be_block::{BlockContent, BlockMetadata};
use be_cli::{cli, tracker::Tracker};
use be_client::{Credentials, Peer, PeerConfig};
use be_frozen::scene;
use be_graph::BlockParent;
use be_store::MemoryStore;
use text_block_types::{TextBlock, TextContent};
use tokio::{net::TcpListener, sync::oneshot};
use uuid::Uuid;

fn main() -> Result<(), String> {
    let Some(file) = std::env::args().nth(1) else {
        return Err(
            "usage: block-file FILE, such as crates/be-frozen/src/fixtures/tracker.block"
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
    build_id::install("block-file");
    let data = std::env::temp_dir().join(format!("be-block-file-{}", Uuid::new_v4()));
    let bytes = tokio::runtime::Runtime::new()
        .map_err(text)?
        .block_on(serve(&data));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::write(&file, bytes?).map_err(text)?;
    println!("wrote {}", file.display());
    Ok(())
}

async fn serve(data: &std::path::Path) -> Result<Vec<u8>, String> {
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(text)?;
    let url = format!("http://{}", listener.local_addr().map_err(text)?);
    let (stop, stopped) = oneshot::channel::<()>();
    let server = tokio::spawn(be_server::serve_with_config(
        listener,
        data.to_path_buf(),
        be_server::ServerConfig::OPEN,
        stopped,
    ));
    let written = write(&url, data).await;
    let _ = stop.send(());
    let _ = tokio::time::timeout(Duration::from_secs(10), server).await;
    written
}

async fn write(url: &str, data: &std::path::Path) -> Result<Vec<u8>, String> {
    let owner = Peer::connect(
        PeerConfig::new(
            be_client::socket_url(url),
            be_keys::RecoveryPhrase::generate().secret(),
            Credentials::Token(
                be_server::ServerStore::open(data)
                    .map_err(text)?
                    .register(scene::OWNER_EMAIL, scene::OWNER_NAME, scene::OWNER_PASSWORD)
                    .await
                    .map_err(text)?
                    .1,
            ),
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
    Ok(be_client::transfer::encode(&blocks))
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

fn sources() -> be_cli::tracker::Sources {
    Box::new(first_party_types::sources)
}
