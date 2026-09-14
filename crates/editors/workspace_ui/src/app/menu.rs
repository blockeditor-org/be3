use std::collections::HashMap;

use block::{BlockParent, BlockReferenceList};
use block_client::ReferenceList;
use block_editor_plugin::{
    block_ui::BlockLabel,
    egui,
    egui_material_icons::icons::{ICON_LINK_OFF, ICON_SHARE},
};
use uuid::Uuid;

use super::{Frame, NO_EDIT_ACCESS};

pub(crate) enum Action {
    Picker,
    SetParent(BlockParent),
    Rename,
    Share,
    Unlink,
    Delete,
}

pub(crate) struct Permissions {
    pub(crate) add: bool,
    pub(crate) edit: bool,
    pub(crate) delete: bool,
    pub(crate) unlink: Result<(), &'static str>,
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    frame: &Frame<'_>,
    parent_candidates: &mut HashMap<Uuid, ReferenceList>,
    subject: Uuid,
    current_parent: BlockParent,
    permissions: Permissions,
    is_reference: bool,
) -> Option<Action> {
    let mut action = None;
    ui.add_enabled_ui(permissions.add, |ui| {
        if ui.button("Add").clicked() {
            action = Some(Action::Picker);
            ui.close();
        }
    })
    .response
    .on_disabled_hover_text(NO_EDIT_ACCESS);
    ui.add_enabled_ui(permissions.edit, |ui| {
        ui.menu_button("Set parent", |ui| {
            if ui
                .add_enabled(
                    current_parent != BlockParent::Root,
                    egui::Button::new("Root"),
                )
                .clicked()
            {
                action = Some(Action::SetParent(BlockParent::Root));
                ui.close();
            }
            if ui
                .add_enabled(
                    current_parent != BlockParent::Orphaned,
                    egui::Button::new("Orphaned"),
                )
                .clicked()
            {
                action = Some(Action::SetParent(BlockParent::Orphaned));
                ui.close();
            }
            ui.separator();
            let backrefs = parent_candidates.entry(subject).or_insert_with(|| {
                frame
                    .client
                    .watch_references(BlockReferenceList::Backrefs(subject))
            });
            let listed = backrefs.read();
            if listed.is_empty() {
                ui.weak(match backrefs.is_loaded() {
                    true => "No backrefs",
                    false => "Loading\u{2026}",
                });
            }
            for backref in listed {
                let is_current = current_parent == BlockParent::Uuid(backref.id);
                let label =
                    BlockLabel::for_reference(frame.types, &backref).widget_text(ui.style());
                if ui
                    .add_enabled(!is_current, egui::Button::new(label))
                    .clicked()
                {
                    action = Some(Action::SetParent(BlockParent::Uuid(backref.id)));
                    ui.close();
                }
            }
        });
    })
    .response
    .on_disabled_hover_text(NO_EDIT_ACCESS);
    if ui
        .add_enabled(permissions.edit, egui::Button::new("Rename"))
        .on_disabled_hover_text(NO_EDIT_ACCESS)
        .clicked()
    {
        action = Some(Action::Rename);
        ui.close();
    }
    if ui
        .add_enabled(
            permissions.edit,
            egui::Button::new(format!("{} Share", ICON_SHARE.codepoint)),
        )
        .on_disabled_hover_text("Only accounts that can edit a block may share it")
        .clicked()
    {
        action = Some(Action::Share);
        ui.close();
    }
    if is_reference {
        let button = ui.add_enabled(
            permissions.unlink.is_ok(),
            egui::Button::new(format!("{} Unlink", ICON_LINK_OFF.codepoint)),
        );
        let clicked = match permissions.unlink {
            Ok(()) => button
                .on_hover_text(
                    "Replace this occurrence with its own copy, unaffected by the original",
                )
                .clicked(),
            Err(hover) => button.on_disabled_hover_text(hover).clicked(),
        };
        if clicked {
            action = Some(Action::Unlink);
            ui.close();
        }
    }
    let delete_label = match is_reference {
        true => "Remove link",
        false => "Delete",
    };
    let delete_text = egui::RichText::new(delete_label);
    let delete_text = match permissions.delete {
        true => delete_text.color(ui.visuals().error_fg_color),
        false => delete_text,
    };
    let response = ui.add_enabled(permissions.delete, egui::Button::new(delete_text));
    let response = match is_reference {
        true => response.on_hover_text(
            "Removes this link only, without creating a copy. The original block is not deleted.",
        ),
        false => response,
    };
    if response.clicked() {
        action = Some(Action::Delete);
        ui.close();
    }
    action
}
