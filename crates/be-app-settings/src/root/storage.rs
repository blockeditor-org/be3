use std::cell::RefCell;

use be_sdk_beui::be_sdk_ui::bytes_label;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{AccountStorage, BlockInfo, BlockQuery, Editor, StorageUsageWatch};

use super::ui::Section;
use uuid::Uuid;

const PADDING: Space = Space::Xl;
const SECTION_SPACING: Space = Space::Xl;
const SPACING: Space = Space::Sm;
const MAP_HEIGHT: Size = Size::Dialog;

const NO_SERVER: &str =
    "Storage is counted by the server, and this device has not heard from it yet.";
const ABOUT_MAP: &str =
    "Each tile is a block, as large as everything it holds; go into one to see what is inside it.";

/// The account's storage against its quota, and its blocks as a treemap by the
/// bytes each tree holds.
#[component]
pub fn StoragePage(editor: Editor) -> NodeId {
    let usage = editor.watch::<StorageUsageWatch>();
    let known = create_memo(move || usage.with(Option::is_some));
    let unknown = create_memo(move || !known.get());
    view! {
        <Scroll>
            <Frame padding=PADDING>
                <List spacing=SECTION_SPACING>
                    <Text style=TextStyle::HEADING>"Storage"</Text>
                    <Show condition={unknown}>
                        <Text style=TextStyle::CAPTION @test_id="settings.storage.unknown">
                            {NO_SERVER}
                        </Text>
                    </Show>
                    <Show condition={known}>
                        <Usage usage />
                    </Show>
                    <Section title="What uses it" caption=ABOUT_MAP>
                        <Frame height=MAP_HEIGHT>
                            <BlocksBySize editor />
                        </Frame>
                    </Section>
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
fn Usage(usage: Memo<Option<AccountStorage>>) -> NodeId {
    let used = create_memo(move || {
        usage.with(|usage| usage.as_ref().map_or(0, |usage| usage.held + usage.pending))
    });
    let quota =
        create_memo(move || usage.with(|usage| usage.as_ref().and_then(|usage| usage.quota)));
    let share = create_memo(move || {
        quota
            .get()
            .filter(|quota| *quota > 0)
            .map(|quota| (used.get() as f64 / quota as f64).min(1.0) as f32)
    });
    let limited = create_memo(move || quota.get().is_some());
    let full = create_memo(move || share.get().is_some_and(|share| share >= 1.0));
    let summary = create_memo(move || match quota.get() {
        Some(quota) => format!("{} of {} used", bytes_label(used.get()), bytes_label(quota)),
        None => format!("{} used, with no limit", bytes_label(used.get())),
    });
    let pending =
        create_memo(move || usage.with(|usage| usage.as_ref().map_or(0, |usage| usage.pending)));
    let waiting = create_memo(move || pending.get() > 0);
    let pending_label = create_memo(move || {
        format!(
            "{} uploaded and not yet kept in a block",
            bytes_label(pending.get())
        )
    });
    let fill = create_memo(move || match full.get() {
        true => ColorRole::Danger,
        false => ColorRole::Accent,
    });
    view! {
        <List spacing=SPACING>
            <Text @test_id="settings.storage.used">{summary}</Text>
            <Show condition={limited}>
                <Progress value={share} label="Storage used" fill />
            </Show>
            <Show condition={full}>
                <Text color=TextColor::Danger>
                    "The quota is full: new uploads are refused until something is deleted."
                </Text>
            </Show>
            <Show condition={waiting}>
                <Text style=TextStyle::CAPTION>{pending_label}</Text>
            </Show>
        </List>
    }
}

/// The account's root blocks, then a block's children once it is gone into,
/// each sized by its tree size from the server. A block's own bytes are a tile
/// of their own beside its children, so a level adds up to the block's size.
#[component]
fn BlocksBySize(editor: Editor) -> NodeId {
    let path = create_signal(Vec::<TreemapItem<Uuid>>::new());
    let level = create_memo(move || path.with(|path| path.last().map(|item| item.key)));
    let listing = create_signal(None::<Memo<Ready<Vec<BlockInfo>>>>);
    let kept: Stored<RefCell<Option<Scope>>> = store(RefCell::new(None));
    let home = owner_scope().expect("the storage page is built inside a scope");
    create_binding(move || {
        let query = match level.get() {
            None => BlockQuery::Roots,
            Some(block) => BlockQuery::Children(block),
        };
        let scope = home.run(Scope::detached);
        let watched = untrack(|| scope.run(|| editor.watch_blocks(query)));
        let previous = kept.value().borrow_mut().replace(scope);
        listing.set(Some(watched));
        drop(previous);
    });
    let items = create_memo(move || {
        let listed = listing
            .get()
            .and_then(|listing| listing.get().ok())
            .unwrap_or_default();
        level_items(
            level.get(),
            &listed,
            level.get().and_then(|block| editor.blocks().info(block)),
        )
    });
    view! {
        <Treemap
            items
            path
            format={bytes_label}
            label="All blocks"
            tile_test_id="settings.storage.block"
            up_test_id="settings.storage.up"
        />
    }
}

/// The tiles of one level: each block listed by its tree size, and inside a
/// block, its own bytes beside its children.
fn level_items(
    level: Option<Uuid>,
    listed: &[BlockInfo],
    holder: Option<BlockInfo>,
) -> Vec<TreemapItem<Uuid>> {
    let mut items: Vec<TreemapItem<Uuid>> = listed
        .iter()
        .filter(|info| info.access.can_view() && !info.deleted)
        .map(|info| TreemapItem {
            key: info.id,
            label: info.file_name().unwrap_or_else(|| "Untitled".to_owned()),
            size: info.tree_size,
            opens: info.tree_size > info.size,
        })
        .collect();
    if let (Some(level), Some(holder)) = (level, holder) {
        items.push(TreemapItem {
            key: level,
            label: format!(
                "{} itself",
                holder
                    .file_name()
                    .unwrap_or_else(|| "This block".to_owned())
            ),
            size: holder.size,
            opens: false,
        });
    }
    items
}
