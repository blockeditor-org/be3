//! What the filter finds in content (plans/search.md): the search index's hits for the filter's
//! words, under the rows whose names match, each with its name and the text around the match.

use be_sdk_beui::beui::styled::Span;
use be_sdk_beui::beui::NodeId;
use beui::{Align, Direction, Overflow};
use be_sdk_beui::beui::icons::ICON_FILE_TEXT;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{BlockQuery, Editor, Search, SearchHit};
use uuid::Uuid;

use super::ui::PADDING;

/// The index's hits for what the filter holds, none while it is empty or unanswered.
pub(crate) fn content_hits(editor: Editor, filter: Memo<String>) -> Memo<Vec<SearchHit>> {
    let search = create_memo(move || {
        let text = filter.get();
        (!text.is_empty()).then_some(Search { text, within: None })
    });
    let results = editor.search(search);
    create_memo(move || {
        results
            .get()
            .map(|results| results.hits)
            .unwrap_or_default()
    })
}

#[component]
pub(crate) fn ContentHits(editor: Editor, hits: Memo<Vec<SearchHit>>) -> NodeId {
    let keys = create_memo(move || {
        hits.with(|hits| {
            hits.iter()
                .map(|hit| (hit.block, hit.key.clone()))
                .collect::<Vec<_>>()
        })
    });
    let shown = create_memo(move || !hits.with(Vec::is_empty));
    view! {
        <List spacing=Space::None>
            <Show condition={shown}>
                <Frame padding={Padding::ZERO.top(PADDING).left(PADDING).right(PADDING)}>
                    <Text style=TextStyle::CAPTION @test_id={"file-tree.content-hits"}>
                        "In content"
                    </Text>
                </Frame>
                <Frame padding={Padding::ZERO.top(Space::Xs2).left(PADDING).right(PADDING)}>
                    <List spacing=Space::Xs2>
                        <ForEach keys={keys}>|(block, key): (Uuid, String)| {
                            let hit = create_memo(move || {
                                hits.with(|hits| {
                                    hits.iter()
                                        .find(|hit| hit.block == block && hit.key == key)
                                        .cloned()
                                        .unwrap_or_default()
                                })
                            });
                            view! {
                                <HitRow editor hit block />
                            }
                        }</ForEach>
                    </List>
                </Frame>
            </Show>
        </List>
    }
}

#[component]
fn HitRow(editor: Editor, hit: Memo<SearchHit>, block: Uuid) -> NodeId {
    let info = editor.watch_blocks(BlockQuery::Block(block));
    let found = create_memo(move || info.get().ok().and_then(|infos| infos.first().cloned()));
    let label = create_memo(move || {
        found
            .get()
            .and_then(|info| info.name())
            .unwrap_or_else(|| "Untitled".to_owned())
    });
    // The snippet around its first match, which is marked as the tree marks a name's.
    let parts = create_memo(move || {
        let hit = hit.get();
        match hit.matches.first() {
            Some(range) => (
                hit.snippet[..range.start].to_owned(),
                hit.snippet[range.clone()].to_owned(),
                hit.snippet[range.end..].to_owned(),
            ),
            None => (hit.snippet, String::new(), String::new()),
        }
    });
    let before = create_memo(move || parts.get().0);
    let matched = create_memo(move || parts.get().1);
    let after = create_memo(move || parts.get().2);
    let open = move || {
        if let Some(info) = found.get_untracked() {
            editor.host().open_block(block, info.block_type);
        }
    };
    view! {
        <ListRow
            label
            on_click={open}
            @test_id={format!("file-tree.hit.{block}")}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                <Text color=TextColor::Muted>
                    <Icon glyph={ICON_FILE_TEXT} />
                </Text>
                <ListItem grow=1.0>
                    <List spacing=Space::None>
                        <Text overflow=Overflow::Ellipsis>{label}</Text>
                        <Text style=TextStyle::CAPTION overflow=Overflow::Ellipsis>
                            {before}
                            <Span background=ColorRole::AccentSoft>{matched}</Span>
                            {after}
                        </Text>
                    </List>
                </ListItem>
            </List>
        </ListRow>
    }
}
