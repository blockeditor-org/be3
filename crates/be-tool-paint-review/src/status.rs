use beui::icons::Glyph;
use beui::icons::{ICON_CIRCLE_MINUS, ICON_CIRCLE_PLUS, ICON_DIFF, ICON_IMAGE_OFF};
use beui::styled::*;

use be_tool_paint_review_manifest::Status;

pub(crate) fn label(status: Status) -> &'static str {
    match status {
        Status::Changed => "Changed",
        Status::Added => "Added",
        Status::Removed => "Removed",
        Status::Unreadable => "Unreadable",
    }
}

pub(crate) fn glyph(status: Status) -> Glyph {
    match status {
        Status::Changed => ICON_DIFF,
        Status::Added => ICON_CIRCLE_PLUS,
        Status::Removed => ICON_CIRCLE_MINUS,
        Status::Unreadable => ICON_IMAGE_OFF,
    }
}

pub(crate) fn color(status: Status) -> TextColor {
    match status {
        Status::Changed => TextColor::Accent,
        Status::Added => TextColor::Success,
        Status::Removed => TextColor::Danger,
        Status::Unreadable => TextColor::Warning,
    }
}

pub(crate) fn role(status: Status) -> ColorRole {
    match status {
        Status::Changed => ColorRole::Accent,
        Status::Added => ColorRole::Success,
        Status::Removed => ColorRole::Danger,
        Status::Unreadable => ColorRole::Warning,
    }
}
