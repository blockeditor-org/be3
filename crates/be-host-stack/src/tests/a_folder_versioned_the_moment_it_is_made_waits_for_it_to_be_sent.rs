use super::*;

/// A version command reads the tree it versions from the server: one given right after the tree
/// was made waits for the drain to send all of it, not only the repository it names.
#[test]
fn a_folder_versioned_the_moment_it_is_made_waits_for_it_to_be_sent() {
    let harness = Harness::start();
    harness.connect();
    wait_until("loaded the graph", |shared| {
        shared.graph.ready(super::graph::Query::Roots)
    });
    let (folder, note, repository) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    create(
        repository,
        be_block::RepositoryContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("History"),
        Some(be_block::RepositoryContent::default().encode()),
        false,
    );
    flush_and_send();
    show_version(repository, be_block::RepositoryContent::CONTENT_TYPE);

    ask_for(vec![Query::Roots, Query::Subtree(folder)]);
    let mut listing = be_type_workspace::FolderContent::default();
    listing.apply(&be_type_workspace::Folder::default().add(note));
    create(
        folder,
        be_type_workspace::FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Notes"),
        Some(listing.encode()),
        false,
    );
    create(
        note,
        be_type_text::TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(folder),
        be_block_content::BlockMetadata::named("Note"),
        Some(be_type_text::TextBlock::of("hello\n").encode()),
        false,
    );
    version(
        repository,
        be_sdk_api::VersionCommand::Adopt {
            block_id: folder.into_bytes(),
        },
    );
    let error = crate::wait_until(|shared| {
        version_status(shared, repository)
            .filter(|status| {
                !status.busy && (!status.branches.is_empty() || status.error.is_some())
            })
            .map(|status| status.error.clone())
    });
    assert_eq!(
        error, None,
        "the folder was versioned once the server had it"
    );
}
