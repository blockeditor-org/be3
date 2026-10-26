use block_editor_beui::be_block::{BlockContent, Home, PREFERRED_EDITOR, TAG_COLOR, TagsContent};
use block_editor_beui::beui::icons::{
    ICON_APP_WINDOW, ICON_HOUSE, ICON_LAYOUT_TEMPLATE, ICON_PIN, ICON_SETTINGS, ICON_TAG,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::tags::{tag_color, tags_of};
use block_editor_beui::{BlockQuery, Editor};
use uuid::Uuid;

use super::rows::Row;

fn shown_block(row: Memo<Option<Row>>) -> Option<(Uuid, Uuid)> {
    row.get_untracked()
        .filter(|row| row.access.can_view())
        .and_then(|row| Some((row.id?, row.block_type)))
}

#[component]
pub(crate) fn TagsMenu(editor: Editor, row: Memo<Option<Row>>) -> MenuItem {
    let block = shown_block(row).map(|(id, _)| id);
    let keys = create_memo(move || {
        block.map_or_else(Vec::new, |block| editor.available_tags(block).get())
    });
    let unchangeable = create_memo(move || !row.get().is_some_and(|row| row.can_edit));
    let manage = move || {
        let Some(block) = block else {
            return;
        };
        let tags = editor
            .workspace_tags(block)
            .or_else(|| editor.personal_tags());
        if let Some(tags) = tags {
            editor.host().open_block(tags, TagsContent::CONTENT_TYPE);
        }
    };
    view! {
        <MenuItem label="Tags" glyph=ICON_TAG row_test_id={"file-tree.tags".to_owned()}>
            <ForEach keys={keys}>|tag: Uuid| {
                let found = editor.watch_blocks(BlockQuery::Block(tag));
                let theme = use_theme();
                let info = create_memo(move || found.get().ok().and_then(|found| found.into_iter().next()));
                let label = create_memo(move || {
                    info.get()
                        .and_then(|info| info.name())
                        .unwrap_or_else(|| "Tag".to_owned())
                });
                let color = create_memo(move || {
                    let held = info.get().and_then(|info| info.metadata.get(&TAG_COLOR)).unwrap_or_default();
                    Some(tag_color(held, theme.kind.get()))
                });
                let on = create_memo(move || {
                    block.is_some_and(|block| {
                        editor
                            .blocks()
                            .info(block)
                            .is_some_and(|info| tags_of(&info.metadata).contains(&tag))
                    })
                });
                let checked = create_memo(move || Some(on.get()));
                view! {
                    <MenuItem
                        label={label}
                        glyph=ICON_TAG
                        glyph_color={color}
                        checked={checked}
                        disabled={unchangeable}
                        row_test_id={format!("file-tree.tags.{tag}")}
                        on_click={move || {
                            if let Some(block) = block {
                                editor.set_tagged(block, tag, !on.get_untracked());
                            }
                        }}
                    />
                }
            }</ForEach>
            <MenuItem
                label="Manage tags…"
                glyph=ICON_SETTINGS
                separated=true
                row_test_id={"file-tree.tags.manage".to_owned()}
                on_click={manage}
            />
        </MenuItem>
    }
}

#[component]
pub(crate) fn OpensIn(editor: Editor, row: Memo<Option<Row>>) -> MenuItem {
    let block = shown_block(row);
    let file_name = row.get_untracked().and_then(|row| row.file_name);
    let editors = create_memo(move || {
        let types = editor.block_types();
        block.map_or_else(Vec::new, |(_, block_type)| {
            types
                .editors_for(block_type, file_name.as_deref())
                .into_iter()
                .map(|entry| (entry.editor, entry.name.clone()))
                .collect::<Vec<_>>()
        })
    });
    let keys = create_memo(move || {
        let mut keys = vec![Uuid::nil()];
        keys.extend(editors.with(|editors| editors.iter().map(|(id, _)| *id).collect::<Vec<_>>()));
        keys
    });
    let preferred = create_memo(move || {
        block.and_then(|(id, _)| editor.blocks().info(id)?.metadata.get(&PREFERRED_EDITOR))
    });
    view! {
        <MenuItem
            label="Opens in"
            glyph=ICON_APP_WINDOW
            row_test_id={"file-tree.opens-in".to_owned()}
        >
            <ForEach keys={keys}>|chosen: Uuid| {
                let label = create_memo(move || match chosen.is_nil() {
                    true => "Your default editor".to_owned(),
                    false => editors.with(|editors| {
                        editors
                            .iter()
                            .find(|(id, _)| *id == chosen)
                            .map(|(_, name)| name.clone())
                            .unwrap_or_default()
                    }),
                });
                let checked = create_memo(move || {
                    Some(preferred.get().unwrap_or_default() == chosen)
                });
                view! {
                    <MenuItem
                        label={label}
                        checked={checked}
                        radio=true
                        row_test_id={format!("file-tree.opens-in.{chosen}")}
                        on_click={move || {
                            if let Some((id, _)) = block {
                                let editor_id = (!chosen.is_nil()).then_some(chosen);
                                editor
                                    .blocks()
                                    .define(id, PREFERRED_EDITOR.definition(editor_id.as_ref()));
                            }
                        }}
                    />
                }
            }</ForEach>
        </MenuItem>
    }
}

#[component]
pub(crate) fn HomeItems(editor: Editor, row: Memo<Option<Row>>) -> MenuItem {
    let block = shown_block(row).map(|(id, _)| id);
    let workspace = block.and_then(|block| untrack(|| editor.workspace_of(block)));
    let held = create_memo(move || {
        workspace
            .map(|workspace| {
                editor
                    .resolved_settings::<Home>(workspace)
                    .with(|home| home.value())
            })
            .unwrap_or_default()
    });
    let pinned = create_memo(move || {
        block.is_some_and(|block| held.with(|held| held.pinned.contains_key(&block)))
    });
    let landing = create_memo(move || {
        block.is_some_and(|block| {
            held.with(|held| held.landing.map(|landing| landing.0) == Some(block))
        })
    });
    let template = create_memo(move || {
        block.is_some_and(|block| held.with(|held| held.templates.contains_key(&block)))
    });
    let change = move |edit: block_editor_beui::be_block::Edit| {
        if let Some(workspace) = workspace {
            editor.change_block_kind::<Home>(workspace, edit);
        }
    };
    let pin_label = create_memo(move || match pinned.get() {
        true => "Unpin from the workspace".to_owned(),
        false => "Pin to the workspace".to_owned(),
    });
    let landing_label = create_memo(move || match landing.get() {
        true => "Stop opening it first".to_owned(),
        false => "Open it first in the workspace".to_owned(),
    });
    let template_label = create_memo(move || match template.get() {
        true => "Stop using it as a template".to_owned(),
        false => "Use it as a template".to_owned(),
    });
    view! {
        <MenuItem label="Workspace home" glyph=ICON_HOUSE row_test_id={"file-tree.home".to_owned()}>
            <MenuItem
                label={pin_label}
                glyph=ICON_PIN
                row_test_id={"file-tree.home.pin".to_owned()}
                on_click={move || {
                    if let Some(block) = block {
                        change(Home::pin(block, !pinned.get_untracked()));
                    }
                }}
            />
            <MenuItem
                label={landing_label}
                glyph=ICON_HOUSE
                row_test_id={"file-tree.home.landing".to_owned()}
                on_click={move || {
                    change(Home::set_landing(match landing.get_untracked() {
                        true => None,
                        false => block,
                    }));
                }}
            />
            <MenuItem
                label={template_label}
                glyph=ICON_LAYOUT_TEMPLATE
                row_test_id={"file-tree.home.template".to_owned()}
                on_click={move || {
                    if let Some(block) = block {
                        change(Home::keep_template(block, !template.get_untracked()));
                    }
                }}
            />
        </MenuItem>
    }
}
