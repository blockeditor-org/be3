use super::*;
use be_block_content::ChildChange;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

/// An index whose head moves runs a standing search again (`forget_answers`): until the new
/// answer has its snippets the one before stays shown, not complete, so a page reading in that
/// gap never sees the hits go.
#[test]
fn a_search_shows_its_answer_while_it_runs_again() {
    let harness = Harness::start();
    harness.connect();
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Orchard"),
        None,
        false,
    );
    wait_until("knew the folder's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    let note = Uuid::new_v4();
    create(
        note,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block_content::BlockMetadata::named("Notes"),
        Some(TextBlock::of("Quinces ripen late").encode()),
        false,
    );
    assert!(made(change_child(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        ChildChange::Add(note)
    )));
    flush();
    let ask = SearchAsk {
        text: "ripen".to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    // From the first complete answer on, every answer read holds the hit.
    let found = std::cell::Cell::new(false);
    let snippet = |shared: &Shared| {
        let answer = shared.search.answer(&ask)?;
        if found.get() {
            assert!(
                !answer.hits.is_empty(),
                "the answer showed no hits: {answer:?}"
            );
        }
        let first = answer.hits.first()?.snippet.clone();
        found.set(found.get() || answer.complete);
        Some((answer.complete, first))
    };
    // Made here, the note is queued for its index at once: no answer is complete without it.
    crate::wait_until("found the note", |shared| {
        if let Some(answer) = shared.search.answer(&ask) {
            assert!(
                !answer.complete || !answer.hits.is_empty(),
                "a complete answer left out the note made here: {answer:?}"
            );
        }
        snippet(shared).filter(|(complete, _)| *complete)
    });

    // The edit moves the index's head; every answer read meanwhile still holds the hit.
    open(note, TextContent::CONTENT_TYPE);
    type_text(note, 0, "Medlars and ");
    flush();
    crate::wait_until("found the note as edited", |shared| {
        snippet(shared).filter(|(complete, text)| *complete && text.contains("Medlars"))
    });
}
