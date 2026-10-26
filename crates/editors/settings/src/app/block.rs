use block_editor_beui::be_block::{
    ANYONE_INSIDE, BlockContent, EDIT_KINDS, SettingsContent, WORKSPACE_TYPE,
};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_EXTERNAL_LINK, ICON_TRASH};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::block_ui::BlockTypes;
use block_editor_beui::{AccessLevel, AccessListing, BlockParent, BlockQuery, Editor, SetKind};
use uuid::Uuid;

const NOTHING_HERE: &str =
    "Nothing is set here yet. Editors set what applies inside, such as a game world's hotbar.";
const ONLY_MANAGERS: &str = "Only the people who manage it change these.";
const ANYONE_INSIDE_LABEL: &str = "Anyone inside";
const MEMBERS_ONLY_LABEL: &str = "Members only";

pub(super) fn name_of(editor: Editor, block: Uuid) -> String {
    let types = editor.block_types();
    editor.blocks().info(block).map_or_else(
        || "a block you can't open".to_owned(),
        |info| info.label(types.as_ref()).name,
    )
}

fn kind_name(editor: Editor, kind: Uuid) -> String {
    editor
        .block_types()
        .display_name(kind)
        .unwrap_or("Settings")
        .to_owned()
}

#[component]
pub(super) fn BlockSettingsView(editor: Editor, owner: Uuid) -> NodeId {
    let settings = editor.block_content::<SettingsContent>();
    let blocks = editor.blocks();
    let _watched = store((
        blocks.watch(BlockQuery::Block(owner)),
        blocks.watch(BlockQuery::Parents(owner)),
    ));
    let in_settings = settings.project(|settings| {
        settings
            .root()
            .kinds
            .iter()
            .map(|(kind, block)| (*kind, *block))
            .collect::<Vec<_>>()
    });
    let here = create_memo(move || {
        let mut here: Vec<(Uuid, Uuid)> = EDIT_KINDS
            .iter()
            .filter_map(|kind| Some((*kind, editor.edit_kind(owner, *kind)?)))
            .collect();
        here.extend(in_settings.get());
        here
    });
    let kinds_here = create_memo(move || {
        here.get()
            .into_iter()
            .map(|(kind, _)| kind)
            .collect::<Vec<_>>()
    });
    let manages = create_memo(move || editor.blocks().access(owner).can_manage());
    let title = create_memo(move || name_of(editor, owner));
    let description = create_memo(move || {
        let kind = match editor
            .blocks()
            .info(owner)
            .is_some_and(|info| info.block_type == WORKSPACE_TYPE)
        {
            true => "workspace",
            false => "block",
        };
        let changed = match manages.get() {
            true => String::new(),
            false => format!(" {ONLY_MANAGERS}"),
        };
        format!(
            "What is set here applies to everything inside this {kind}, unless a block inside sets its own.{changed}"
        )
    });
    let parent = create_memo(move || {
        editor
            .blocks()
            .info(owner)
            .and_then(|info| info.parent?.block())
    });
    let above = create_memo(move || {
        let Some(parent) = parent.get() else {
            return Vec::new();
        };
        let shown = kinds_here.get();
        editor.kinds_above(parent).with(|kinds| {
            kinds
                .iter()
                .filter(|(kind, _)| !shown.contains(kind))
                .map(|(kind, set)| (*kind, *set))
                .collect::<Vec<_>>()
        })
    });
    let above_kinds = create_memo(move || {
        above
            .get()
            .into_iter()
            .map(|(kind, _)| kind)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || kinds_here.with(Vec::is_empty));
    let inherits = create_memo(move || !above.with(Vec::is_empty));
    view! {
        <SettingsPage>
            <SettingsSection title={title} description={description}>
                <Show condition={empty}>
                    <SettingsRow label={NOTHING_HERE.to_owned()} compact=true />
                </Show>
                <ForEach keys={kinds_here}>|kind: Uuid| {
                    let block = create_memo(move || {
                        here.with(|here| {
                            here.iter()
                                .find(|(found, _)| *found == kind)
                                .map(|(_, block)| *block)
                        })
                    });
                    let changes = create_memo(move || match EDIT_KINDS.contains(&kind) {
                        true => editor.blocks().access(owner).can_edit(),
                        false => manages.get(),
                    });
                    view! {
                        <KindHere editor={editor} kind={kind} block={block} manages={changes} />
                    }
                }</ForEach>
            </SettingsSection>
            <Show condition={inherits}>
                <SettingsSection
                    title="From above"
                    description="Set on a folder or workspace this is inside. What is set here replaces it."
                >
                    <ForEach keys={above_kinds}>|kind: Uuid| {
                        let set = create_memo(move || {
                            above.with(|above| {
                                above
                                    .iter()
                                    .find(|(found, _)| *found == kind)
                                    .map(|(_, set)| *set)
                            })
                        });
                        view! {
                            <KindAbove editor={editor} kind={kind} set={set} />
                        }
                    }</ForEach>
                </SettingsSection>
            </Show>
        </SettingsPage>
    }
}

#[component]
fn KindHere(editor: Editor, kind: Uuid, block: Memo<Option<Uuid>>, manages: Memo<bool>) -> NodeId {
    let label = create_memo(move || kind_name(editor, kind));
    let open_inside = create_signal(None::<bool>);
    create_effect(move || {
        if let (Some(block), true) = (block.get(), manages.get()) {
            let asked = untrack(|| editor.host().list_access(block));
            editor.then(asked, move |listed| {
                if let Ok(AccessListing { grants, .. }) = listed {
                    let open = grants.iter().any(|grant| {
                        Uuid::from_bytes(grant.account) == ANYONE_INSIDE
                            && grant.granted.is_some_and(|level| level.can_view())
                    });
                    open_inside.set(Some(open));
                }
            });
        }
    });
    let selected = create_memo(move || {
        open_inside.get().map(|open| match open {
            true => 0,
            false => 1,
        })
    });
    let choose = move |index: Option<usize>| {
        let (Some(index), Some(block)) = (index, block.get_untracked()) else {
            return;
        };
        let open = index == 0;
        open_inside.set(Some(open));
        let level = match open {
            true => AccessLevel::View,
            false => AccessLevel::None,
        };
        editor.host().set_access(block, ANYONE_INSIDE, level);
    };
    let unchangeable = create_memo(move || !manages.get() || open_inside.get().is_none());
    let unremovable = create_memo(move || !manages.get());
    let open = move || {
        if let Some(block) = block.get_untracked() {
            editor.host().open_block(block, kind);
        }
    };
    let remove = move || editor.clear_block_kind(editor.block_id(), kind);
    view! {
        <SettingsRow label={label} description={"Set here".to_owned()}>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Select
                    label="Who can read it"
                    options={view! {
                        <ChoiceOption label={ANYONE_INSIDE_LABEL.to_owned()} />
                        <ChoiceOption label={MEMBERS_ONLY_LABEL.to_owned()} />
                    }}
                    selected={Binding::new(selected, choose)}
                    disabled={unchangeable}
                    @test_id={format!("settings.kind.{kind}.visibility")}
                />
                <Button
                    icon_only=true
                    glyph=ICON_EXTERNAL_LINK
                    label="Open"
                    @test_id={format!("settings.kind.{kind}.open")}
                    on_click={open}
                />
                <Button
                    icon_only=true
                    glyph=ICON_TRASH
                    label="Remove"
                    disabled={unremovable}
                    @test_id={format!("settings.kind.{kind}.remove")}
                    on_click={remove}
                />
            </List>
        </SettingsRow>
    }
}

#[component]
fn KindAbove(editor: Editor, kind: Uuid, set: Memo<Option<SetKind>>) -> NodeId {
    let label = create_memo(move || kind_name(editor, kind));
    let source = create_memo(move || {
        set.get().map_or_else(String::new, |set| {
            format!("Set in {}", name_of(editor, set.owner))
        })
    });
    let open = move || {
        let Some(set) = set.get_untracked() else {
            return;
        };
        match EDIT_KINDS.contains(&kind) {
            true => editor.host().open_block(set.block, kind),
            false => editor
                .host()
                .open_block(set.settings, SettingsContent::CONTENT_TYPE),
        }
    };
    view! {
        <SettingsRow label={label} description={source}>
            <Button
                label="Open its settings"
                variant=ButtonVariant::Ghost
                @test_id={format!("settings.above.{kind}.open")}
                on_click={open}
            />
        </SettingsRow>
    }
}

pub(super) fn open_settings_of(editor: Editor, owner: Uuid) {
    if let Some(settings) = editor.ensure_settings_block(owner) {
        editor
            .host()
            .open_block(settings, SettingsContent::CONTENT_TYPE);
    }
}

pub(super) fn is_block_settings(editor: Editor, block: Uuid) -> Option<Option<Uuid>> {
    let info = editor.blocks().info(block)?;
    Some(match info.parent {
        Some(BlockParent::Block(owner)) if info.block_type == SettingsContent::CONTENT_TYPE => {
            Some(owner)
        }
        _ => None,
    })
}
