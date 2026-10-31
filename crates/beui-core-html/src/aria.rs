use accesskit::{HasPopup, Orientation, Role, Toggled};

#[derive(Clone, Copy, PartialEq)]
enum Toggle {
    Checked,
    Pressed,
}

struct Mapping {
    role: Role,
    aria: &'static str,
    native: &'static [&'static str],
    toggle: Option<Toggle>,
    range: bool,
}

const fn plain(role: Role, aria: &'static str) -> Mapping {
    Mapping {
        role,
        aria,
        native: &[],
        toggle: None,
        range: false,
    }
}

const fn native(role: Role, aria: &'static str, native: &'static [&'static str]) -> Mapping {
    Mapping {
        role,
        aria,
        native,
        toggle: None,
        range: false,
    }
}

const fn checkable(role: Role, aria: &'static str) -> Mapping {
    Mapping {
        role,
        aria,
        native: &[],
        toggle: Some(Toggle::Checked),
        range: false,
    }
}

const fn range(role: Role, aria: &'static str) -> Mapping {
    Mapping {
        role,
        aria,
        native: &[],
        toggle: None,
        range: true,
    }
}

const TEXT_INPUT: &[&str] = &["input", "textarea"];

const BUTTON: Mapping = Mapping {
    role: Role::Button,
    aria: "button",
    native: &["button"],
    toggle: Some(Toggle::Pressed),
    range: false,
};

const MAPPINGS: &[Mapping] = &[
    BUTTON,
    Mapping {
        role: Role::DefaultButton,
        ..BUTTON
    },
    Mapping {
        role: Role::DisclosureTriangle,
        native: &["summary"],
        ..BUTTON
    },
    native(Role::Link, "link", &["a"]),
    checkable(Role::CheckBox, "checkbox"),
    checkable(Role::Switch, "switch"),
    checkable(Role::RadioButton, "radio"),
    plain(Role::RadioGroup, "radiogroup"),
    plain(Role::Tab, "tab"),
    plain(Role::TabList, "tablist"),
    plain(Role::TabPanel, "tabpanel"),
    plain(Role::Menu, "menu"),
    plain(Role::MenuListPopup, "menu"),
    plain(Role::MenuBar, "menubar"),
    plain(Role::MenuItem, "menuitem"),
    checkable(Role::MenuItemCheckBox, "menuitemcheckbox"),
    checkable(Role::MenuItemRadio, "menuitemradio"),
    plain(Role::ListBox, "listbox"),
    plain(Role::ListBoxOption, "option"),
    plain(Role::MenuListOption, "option"),
    native(Role::List, "list", &["ul", "ol"]),
    native(Role::ListItem, "listitem", &["li"]),
    plain(Role::Tree, "tree"),
    plain(Role::TreeItem, "treeitem"),
    plain(Role::TreeGrid, "treegrid"),
    plain(Role::Grid, "grid"),
    native(Role::Table, "table", &["table"]),
    plain(Role::RowGroup, "rowgroup"),
    native(Role::Row, "row", &["tr"]),
    native(Role::Cell, "cell", &["td"]),
    plain(Role::GridCell, "gridcell"),
    native(Role::ColumnHeader, "columnheader", &["th"]),
    plain(Role::RowHeader, "rowheader"),
    native(Role::Dialog, "dialog", &["dialog"]),
    plain(Role::AlertDialog, "alertdialog"),
    plain(Role::Alert, "alert"),
    plain(Role::Status, "status"),
    plain(Role::Log, "log"),
    plain(Role::Timer, "timer"),
    plain(Role::Marquee, "marquee"),
    plain(Role::Tooltip, "tooltip"),
    plain(Role::Toolbar, "toolbar"),
    plain(Role::Group, "group"),
    native(
        Role::Heading,
        "heading",
        &["h1", "h2", "h3", "h4", "h5", "h6"],
    ),
    native(Role::Image, "img", &["img"]),
    plain(Role::Figure, "figure"),
    plain(Role::Navigation, "navigation"),
    plain(Role::Main, "main"),
    plain(Role::Banner, "banner"),
    plain(Role::Complementary, "complementary"),
    plain(Role::ContentInfo, "contentinfo"),
    plain(Role::Region, "region"),
    plain(Role::Form, "form"),
    plain(Role::Search, "search"),
    plain(Role::Document, "document"),
    plain(Role::Application, "application"),
    plain(Role::Article, "article"),
    plain(Role::Note, "note"),
    plain(Role::Feed, "feed"),
    plain(Role::Paragraph, "paragraph"),
    plain(Role::Terminal, "log"),
    range(Role::Slider, "slider"),
    range(Role::SpinButton, "spinbutton"),
    range(Role::NumberInput, "spinbutton"),
    range(Role::ProgressIndicator, "progressbar"),
    range(Role::Meter, "meter"),
    range(Role::ScrollBar, "scrollbar"),
    range(Role::Splitter, "separator"),
    native(Role::TextInput, "textbox", TEXT_INPUT),
    native(Role::MultilineTextInput, "textbox", TEXT_INPUT),
    native(Role::EmailInput, "textbox", TEXT_INPUT),
    native(Role::PasswordInput, "textbox", TEXT_INPUT),
    native(Role::PhoneNumberInput, "textbox", TEXT_INPUT),
    native(Role::UrlInput, "textbox", TEXT_INPUT),
    native(Role::DateInput, "textbox", TEXT_INPUT),
    native(Role::DateTimeInput, "textbox", TEXT_INPUT),
    native(Role::WeekInput, "textbox", TEXT_INPUT),
    native(Role::MonthInput, "textbox", TEXT_INPUT),
    native(Role::TimeInput, "textbox", TEXT_INPUT),
    native(Role::SearchInput, "searchbox", TEXT_INPUT),
    native(Role::ComboBox, "combobox", &["select"]),
    native(Role::EditableComboBox, "combobox", TEXT_INPUT),
];

pub(crate) fn attributes(node: &accesskit::Node, tag: Option<&str>) -> Vec<(&'static str, String)> {
    let Some(mapping) = MAPPINGS.iter().find(|mapping| mapping.role == node.role()) else {
        return Vec::new();
    };
    let native = tag.is_some_and(|tag| {
        mapping
            .native
            .iter()
            .any(|native| native.eq_ignore_ascii_case(tag))
    });
    let flag = |set: bool| set.then(|| "true".to_owned());
    let boolean = |value: Option<bool>| value.map(|value| value.to_string());
    let number =
        |value: Option<f64>| value.map(|value| ((value * 1000.0).round() / 1000.0).to_string());
    let toggled = node.toggled().map(|toggled| {
        match toggled {
            Toggled::True => "true",
            Toggled::False => "false",
            Toggled::Mixed => "mixed",
        }
        .to_owned()
    });
    let toggle = |which: Toggle| toggled.clone().filter(|_| mapping.toggle == Some(which));
    let ranged = |value: Option<String>| value.filter(|_| mapping.range);
    let text_input = mapping.native == TEXT_INPUT;
    [
        ("role", (!native).then(|| mapping.aria.to_owned())),
        ("aria-label", node.label().map(str::to_owned)),
        (
            "aria-level",
            (node.role() == Role::Heading)
                .then(|| node.level().unwrap_or(2).clamp(1, 6).to_string()),
        ),
        ("aria-checked", toggle(Toggle::Checked)),
        ("aria-pressed", toggle(Toggle::Pressed)),
        ("aria-selected", boolean(node.is_selected())),
        ("aria-expanded", boolean(node.is_expanded())),
        ("aria-disabled", flag(node.is_disabled())),
        ("aria-valuenow", ranged(number(node.numeric_value()))),
        ("aria-valuemin", ranged(number(node.min_numeric_value()))),
        ("aria-valuemax", ranged(number(node.max_numeric_value()))),
        ("aria-valuetext", ranged(node.value().map(str::to_owned))),
        (
            "aria-orientation",
            node.orientation().map(|orientation| {
                match orientation {
                    Orientation::Horizontal => "horizontal",
                    Orientation::Vertical => "vertical",
                }
                .to_owned()
            }),
        ),
        (
            "aria-haspopup",
            node.has_popup().map(|popup| {
                match popup {
                    HasPopup::Menu => "menu",
                    HasPopup::Listbox => "listbox",
                    HasPopup::Tree => "tree",
                    HasPopup::Grid => "grid",
                    HasPopup::Dialog => "dialog",
                }
                .to_owned()
            }),
        ),
        ("aria-multiselectable", flag(node.is_multiselectable())),
        ("aria-modal", flag(node.is_modal())),
        ("aria-readonly", flag(node.is_read_only())),
        ("aria-required", flag(node.is_required())),
        (
            "aria-multiline",
            flag(!native && node.role() == Role::MultilineTextInput),
        ),
        (
            "aria-placeholder",
            node.placeholder()
                .filter(|_| text_input && !native)
                .map(str::to_owned),
        ),
    ]
    .into_iter()
    .filter_map(|(name, value)| Some((name, value?)))
    .collect()
}

#[cfg(test)]
mod tests;
