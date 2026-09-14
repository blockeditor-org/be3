use block::{BlockAccess, BlockParent};
use block_client::ReferenceList;
use block_editor_plugin::{
    AccessLevel,
    block_ui::{BlockLabel, test_id::TestId},
    egui,
    egui_material_icons::icons::{
        ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_AUTO_AWESOME, ICON_CHEVRON_RIGHT,
        ICON_DATA_OBJECT, ICON_EDIT, ICON_LINK, ICON_LINK_OFF, ICON_LOCK, ICON_REDO, ICON_REFRESH,
        ICON_SETTINGS, ICON_SHARE, ICON_UNDO, ICON_VISIBILITY,
    },
};
use uuid::Uuid;

use super::{Frame, Navigation, State, TabItem, block_data, status};

#[derive(Default)]
pub(crate) struct Outcome {
    pub(crate) navigation: Option<Navigation>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Access(BlockAccess),
    Debug,
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    frame: &Frame<'_>,
    item: TabItem,
    can_go_back: bool,
    can_go_forward: bool,
) -> Outcome {
    let mut outcome = Outcome::default();
    let access = state.access(frame, item.id);
    let ceiling = frame.ceiling(item.id);
    let denied = !access.can_view();
    if !denied {
        artifact_bar(ui, frame, item.id, &mut outcome);
        linked_bar(ui, state, frame, item.id, &mut outcome);
    }
    let label = frame.label(item.id, item.block_type);
    let debug = state.debug_blocks.contains(&item.id);
    let mut mode = match debug {
        true => Mode::Debug,
        false => Mode::Access(access),
    };
    let undo = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
    let redo = egui::KeyboardShortcut::new(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    let redo_y = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Y);
    let undo_requested =
        access.can_edit() && ui.ctx().input_mut(|input| input.consume_shortcut(&undo));
    let redo_requested = access.can_edit()
        && (ui.ctx().input_mut(|input| input.consume_shortcut(&redo))
            || ui.ctx().input_mut(|input| input.consume_shortcut(&redo_y)));
    let mut share = false;
    egui::Sides::new().shrink_left().show(
        ui,
        |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(can_go_back, egui::Button::new(ICON_ARROW_BACK))
                    .test_id("workspace.back")
                    .on_hover_text("Back")
                    .clicked()
                {
                    outcome.navigation = Some(Navigation::Back);
                }
                if ui
                    .add_enabled(can_go_forward, egui::Button::new(ICON_ARROW_FORWARD))
                    .test_id("workspace.forward")
                    .on_hover_text("Forward")
                    .clicked()
                {
                    outcome.navigation = Some(Navigation::Forward);
                }
                ui.separator();
                let handle = state.handle(item);
                let history = handle
                    .and_then(|handle| handle.history())
                    .filter(|_| access.can_edit());
                if (ui
                    .add_enabled(
                        history.is_some_and(|history| history.can_undo()),
                        egui::Button::new(ICON_UNDO),
                    )
                    .test_id("workspace.undo")
                    .on_hover_text("Undo (Ctrl/Cmd+Z)")
                    .clicked()
                    || undo_requested)
                    && let Some(history) = history
                {
                    history.undo();
                }
                if (ui
                    .add_enabled(
                        history.is_some_and(|history| history.can_redo()),
                        egui::Button::new(ICON_REDO),
                    )
                    .test_id("workspace.redo")
                    .on_hover_text("Redo (Ctrl+Y or Ctrl/Cmd+Shift+Z)")
                    .clicked()
                    || redo_requested)
                    && let Some(history) = history
                {
                    history.redo();
                }
                ui.separator();
                if let Some(item) = breadcrumbs(ui, state, frame, item.id, &label) {
                    outcome.navigation = Some(Navigation::Open(item));
                }
            });
        },
        |ui| {
            share = ui
                .add_enabled(
                    frame.can_edit(item.id),
                    egui::Button::new(format!("{} Share", ICON_SHARE.codepoint)),
                )
                .test_id("workspace.share")
                .on_hover_text("Share this block")
                .on_disabled_hover_text("Only accounts that can edit a block may share it")
                .clicked();
            mode = access_mode(
                ui,
                item.id,
                mode,
                ceiling,
                frame.client.is_dynamic_artifact(item.id),
            );
        },
    );
    if share {
        frame.host.share_block(item.id);
    }
    match mode {
        Mode::Access(chosen) => {
            state.debug_blocks.remove(&item.id);
            let level = match chosen {
                BlockAccess::Edit => AccessLevel::Edit,
                BlockAccess::View => AccessLevel::View,
                BlockAccess::KnowExists => AccessLevel::KnowExists,
                BlockAccess::None => AccessLevel::None,
            };
            if chosen != access {
                state.simulated.insert(item.id, level);
                frame.host.simulate_access(item.id, level);
            }
        }
        Mode::Debug => {
            state.debug_blocks.insert(item.id);
        }
    }
    ui.separator();
    if state.debug_blocks.contains(&item.id) {
        if ceiling.can_view() {
            block_data::show(ui, frame.client, item.id);
            return outcome;
        }
        state.debug_blocks.remove(&item.id);
    }
    if denied {
        access_denied(ui, ceiling.can_view());
        return outcome;
    }
    let child = frame.host.child(ui, item.id, item.block_type);
    child.keep_active();
    child.own_frame();
    if !child.available() {
        ui.put(
            child.rect(),
            egui::Label::new(child.error().unwrap_or("This block is loading\u{2026}")),
        );
    }
    outcome
}

fn access_denied(ui: &mut egui::Ui, simulated: bool) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.heading(format!("{} No access", ICON_LOCK.codepoint));
            if simulated {
                ui.weak("An account that only knows this block exists cannot open it.");
                ui.weak("Switch back to Can view or Can edit to see it.");
            } else {
                ui.weak("You do not have permission to open this block.");
                ui.weak("Ask someone who can edit it to share it with you.");
            }
        });
    });
}

fn breadcrumbs(
    ui: &mut egui::Ui,
    state: &mut State,
    frame: &Frame<'_>,
    id: Uuid,
    label: &BlockLabel,
) -> Option<TabItem> {
    let mut navigate = None;
    let parents = state.parents.get(&id).map(ReferenceList::read);
    let root = parents
        .as_ref()
        .and_then(|parents| parents.first().map(|parent| parent.parent));
    ui.label(match root {
        Some(BlockParent::Orphaned) => "Recently Deleted",
        Some(BlockParent::Root) => "Root",
        Some(BlockParent::Uuid(_)) | None => "Unknown",
    });
    if let Some(parents) = parents {
        for parent in parents {
            state.record_reference_types(&parent);
            ui.label(ICON_CHEVRON_RIGHT);
            let parent_label =
                BlockLabel::for_reference(frame.types, &parent).widget_text(ui.style());
            if ui
                .button(parent_label)
                .on_hover_text(parent.id.to_string())
                .clicked()
            {
                navigate = Some(TabItem {
                    id: parent.id,
                    block_type: parent.block_type,
                });
            }
        }
    }
    ui.label(ICON_CHEVRON_RIGHT);
    ui.label(label.widget_text(ui.style()));
    navigate
}

fn artifact_bar(ui: &mut egui::Ui, frame: &Frame<'_>, id: Uuid, outcome: &mut Outcome) {
    if !frame.client.is_dynamic_artifact(id) {
        return;
    }
    let artifact = frame.host.artifact(id);
    let can_regenerate = frame.can_edit(id);
    let described = artifact
        .as_ref()
        .is_some_and(|artifact| artifact.source.is_some());
    let running = artifact
        .as_ref()
        .is_some_and(|artifact| artifact.regenerating);
    let mut regenerate = false;
    let mut settings = false;
    let mut unlink = false;
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{} Dynamic artifact", ICON_AUTO_AWESOME.codepoint));
                match &artifact {
                    Some(artifact) if artifact.source.is_some() => {
                        let source = artifact.source.expect("the source was just checked");
                        ui.weak("Generated from");
                        let label = frame.label(source, artifact.source_type);
                        if ui
                            .button(label.widget_text(ui.style()))
                            .on_hover_text(format!("Open the source block\n{source}"))
                            .clicked()
                        {
                            outcome.navigation = Some(Navigation::Open(TabItem {
                                id: source,
                                block_type: artifact.source_type,
                            }));
                        }
                        ui.separator();
                        ui.weak(&artifact.summary);
                        settings = ui
                            .add_enabled(can_regenerate, egui::Button::new(ICON_SETTINGS))
                            .on_hover_text("Settings")
                            .on_disabled_hover_text(super::NO_EDIT_ACCESS)
                            .clicked();
                    }
                    Some(artifact) if artifact.error.is_some() => {
                        let error = artifact.error.clone().unwrap_or_default();
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    Some(_) | None => {
                        ui.spinner();
                    }
                }
                if running {
                    ui.spinner();
                }
                regenerate = ui
                    .add_enabled(
                        can_regenerate && !running && described,
                        egui::Button::new(ICON_REFRESH),
                    )
                    .on_hover_text("Regenerate")
                    .on_disabled_hover_text("You cannot change this block")
                    .clicked();
                unlink = ui
                    .add_enabled(can_regenerate && !running, egui::Button::new(ICON_LINK_OFF))
                    .on_hover_text("Unlink from the source block")
                    .on_disabled_hover_text("You cannot change this block")
                    .clicked();
            });
            if let Some(error) = artifact
                .as_ref()
                .and_then(|artifact| artifact.error.as_ref())
                .filter(|_| described)
            {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
    ui.separator();
    if regenerate {
        frame.host.regenerate_artifact(id);
    }
    if settings {
        frame.host.edit_artifact(id);
    }
    if unlink {
        frame.host.unlink_artifact(id);
    }
}

fn linked_bar(
    ui: &mut egui::Ui,
    state: &mut State,
    frame: &Frame<'_>,
    id: Uuid,
    outcome: &mut Outcome,
) {
    let Some((loaded, backrefs)) = state
        .backrefs
        .get(&id)
        .map(|list| (list.is_loaded(), list.read()))
    else {
        return;
    };
    if !loaded || backrefs.len() <= 1 {
        return;
    }
    let count = backrefs.len();
    let container = state.opened_via.get(&id).copied();
    let parent = state
        .handle(TabItem {
            id,
            block_type: state.block_types.get(&id).copied().unwrap_or_default(),
        })
        .and_then(|handle| handle.relationships())
        .map(|relationships| relationships.parent);
    let Some(parent) = parent else {
        return;
    };
    let via_reference = container.is_some_and(|container| parent != BlockParent::Uuid(container));
    let unlink_permission = status::unlink_permission(state, frame, container);
    let mut go_to_original = false;
    let mut unlink = false;
    let mut navigate = None;
    let mut action = None;
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .inner_margin(egui::Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{} Linked block", ICON_LINK.codepoint));
                if via_reference {
                    let others = count - 1;
                    ui.weak(format!(
                        "This block also appears in {others} other place{}. Editing it here changes it everywhere it appears.",
                        if others == 1 { "" } else { "s" }
                    ));
                    go_to_original = ui.button("Go to original").clicked();
                    let button = ui.add_enabled(
                        unlink_permission.is_ok(),
                        egui::Button::new(format!("{} Unlink", ICON_LINK_OFF.codepoint)),
                    );
                    unlink = match unlink_permission {
                        Ok(()) => button
                            .on_hover_text(
                                "Replace this occurrence with its own copy, unaffected by the original",
                            )
                            .clicked(),
                        Err(hover) => button.on_disabled_hover_text(hover).clicked(),
                    };
                } else {
                    ui.weak(format!(
                        "This block appears in {count} places. Editing it here changes it everywhere."
                    ));
                    ui.menu_button("Show references", |ui| {
                        status::reference_list(
                            ui,
                            state,
                            frame,
                            &backrefs,
                            true,
                            "No backrefs",
                            None,
                            &mut navigate,
                            &mut action,
                        );
                    });
                }
            });
        });
    ui.separator();
    if let Some(action) = action {
        status::apply(state, frame, action);
    }
    if let Some(item) = navigate {
        outcome.navigation = Some(Navigation::Open(item));
    }
    if go_to_original {
        state.opened_via.remove(&id);
    }
    if unlink && let Some(container) = container {
        frame.host.unlink_block(id, container);
    }
}

fn access_mode(
    ui: &mut egui::Ui,
    id: Uuid,
    current: Mode,
    ceiling: BlockAccess,
    generated: bool,
) -> Mode {
    let mut chosen = current;
    egui::ComboBox::from_id_salt(("editor-access-mode", id))
        .selected_text(mode_label(current))
        .show_ui(ui, |ui| {
            for mode in [BlockAccess::Edit, BlockAccess::View, BlockAccess::KnowExists] {
                let label = access_label(mode);
                ui.add_enabled_ui(mode <= ceiling, |ui| {
                    if ui
                        .selectable_label(current == Mode::Access(mode), label)
                        .clicked()
                    {
                        chosen = Mode::Access(mode);
                    }
                })
                .response
                .on_disabled_hover_text(if generated {
                    "Generated blocks are replaced whenever they are rebuilt, so they cannot be edited here"
                } else {
                    "You do not have that much access to this block"
                });
            }
            ui.separator();
            ui.add_enabled_ui(ceiling.can_view(), |ui| {
                if ui
                    .selectable_label(current == Mode::Debug, debug_label())
                    .clicked()
                {
                    chosen = Mode::Debug;
                }
            })
            .response
            .on_disabled_hover_text("You do not have that much access to this block");
        })
        .response
        .on_hover_text(
            "Show this block as an account with this much access would see it, or inspect its raw data",
        );
    chosen
}

fn access_label(access: BlockAccess) -> String {
    let icon = match access {
        BlockAccess::Edit => ICON_EDIT.codepoint,
        BlockAccess::View => ICON_VISIBILITY.codepoint,
        BlockAccess::KnowExists | BlockAccess::None => ICON_LOCK.codepoint,
    };
    format!("{icon} {}", wording(access))
}

fn wording(access: BlockAccess) -> &'static str {
    match access {
        BlockAccess::Edit => "Editing",
        BlockAccess::View => "Viewing",
        BlockAccess::KnowExists | BlockAccess::None => "No access",
    }
}

fn debug_label() -> String {
    format!("{} Debug", ICON_DATA_OBJECT.codepoint)
}

fn mode_label(mode: Mode) -> String {
    match mode {
        Mode::Access(access) => access_label(access),
        Mode::Debug => debug_label(),
    }
}
