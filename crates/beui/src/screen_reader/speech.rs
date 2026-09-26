use accesskit::{Action, Node as AccessNode, NodeId as AccessNodeId, Role, Toggled};

const NAME_LIMIT: usize = 80;

pub(super) type Nodes<'a> = crate::accessibility::AccessibilityView<'a>;

pub(super) fn container(role: Role) -> bool {
    matches!(
        role,
        Role::Window
            | Role::Unknown
            | Role::GenericContainer
            | Role::ScrollView
            | Role::ListBox
            | Role::Tree
            | Role::TabList
            | Role::RadioGroup
            | Role::Group
            | Role::Menu
            | Role::MenuBar
    )
}

pub(super) fn control(node: &AccessNode) -> bool {
    !container(node.role())
        && (node.supports_action(Action::Click)
            || node.supports_action(Action::Focus)
            || node.supports_action(Action::Increment)
            || node.supports_action(Action::SetValue))
}

pub(super) fn scrollable(node: &AccessNode) -> bool {
    node.supports_action(Action::ScrollUp) || node.supports_action(Action::ScrollDown)
}

pub(super) fn adjustable(node: &AccessNode) -> bool {
    node.supports_action(Action::Increment) || node.supports_action(Action::Decrement)
}

pub(super) fn own_name(node: &AccessNode) -> Option<String> {
    node.label()
        .or_else(|| node.value())
        .map(flatten)
        .filter(|text| !text.is_empty())
}

pub(super) fn name(nodes: &Nodes<'_>, node: &AccessNode, derive: bool) -> Option<String> {
    own_name(node).or_else(|| {
        if !derive {
            return None;
        }
        let mut words = Vec::new();
        for child in node.children() {
            gather_text(nodes, *child, &mut words);
        }
        let joined = words.join(" ");
        (!joined.is_empty()).then(|| clip(&joined))
    })
}

fn gather_text(nodes: &Nodes<'_>, id: AccessNodeId, words: &mut Vec<String>) {
    let Some(node) = nodes.get(&id) else {
        return;
    };
    if let Some(text) = node
        .label()
        .or_else(|| node.value())
        .map(flatten)
        .filter(|text| !text.is_empty())
    {
        words.push(text);
    }
    for child in node.children() {
        gather_text(nodes, *child, words);
    }
}

pub(super) fn phrase(nodes: &Nodes<'_>, node: &AccessNode, derive: bool) -> String {
    let mut parts = Vec::new();
    let name = name(nodes, node, derive);
    if let Some(name) = name.clone() {
        parts.push(name);
    }
    parts.push(role_word(node.role()));
    if let Some(value) = node
        .value()
        .map(flatten)
        .filter(|value| !value.is_empty() && Some(value) != name.as_ref())
    {
        parts.push(value);
    }
    parts.extend(states(node));
    if let Some(description) = node
        .description()
        .map(flatten)
        .filter(|text| !text.is_empty())
    {
        parts.push(description);
    }
    parts.join(", ")
}

fn states(node: &AccessNode) -> Vec<String> {
    let mut states = Vec::new();
    let switch = matches!(node.role(), Role::Switch);
    match node.toggled() {
        Some(Toggled::True) if switch => states.push("on".to_owned()),
        Some(Toggled::False) if switch => states.push("off".to_owned()),
        Some(Toggled::True) => states.push("checked".to_owned()),
        Some(Toggled::False) => states.push("not checked".to_owned()),
        Some(Toggled::Mixed) => states.push("partly checked".to_owned()),
        None => {}
    }
    if node.is_selected() == Some(true) {
        states.push("selected".to_owned());
    }
    match node.is_expanded() {
        Some(true) => states.push("expanded".to_owned()),
        Some(false) => states.push("collapsed".to_owned()),
        None => {}
    }
    if let Some(value) = node.numeric_value() {
        states.push(numeric(node, value));
    }
    if let (Some(position), Some(total)) = (node.position_in_set(), node.size_of_set()) {
        states.push(format!("{position} of {total}"));
    }
    if node.is_required() {
        states.push("required".to_owned());
    }
    if node.is_read_only() {
        states.push("read only".to_owned());
    }
    if node.is_disabled() {
        states.push("dimmed".to_owned());
    }
    states
}

fn numeric(node: &AccessNode, value: f64) -> String {
    let (minimum, maximum) = (node.min_numeric_value(), node.max_numeric_value());
    match (minimum, maximum) {
        (Some(minimum), Some(maximum)) if maximum > minimum => {
            let fraction = (value - minimum) / (maximum - minimum);
            format!("{}%", (fraction * 100.0).round())
        }
        _ => format!("{}", (value * 100.0).round() / 100.0),
    }
}

pub(super) fn role_word(role: Role) -> String {
    match role {
        Role::Label => "text".to_owned(),
        Role::GenericContainer | Role::Unknown | Role::Group => "group".to_owned(),
        Role::Window => "window".to_owned(),
        Role::ScrollView => "scroll area".to_owned(),
        Role::DefaultButton => "button".to_owned(),
        Role::TextInput => "text field".to_owned(),
        Role::MultilineTextInput => "text area".to_owned(),
        role => spaced(&format!("{role:?}")),
    }
}

fn spaced(name: &str) -> String {
    let mut out = String::new();
    for (index, letter) in name.chars().enumerate() {
        if letter.is_uppercase() && index > 0 {
            out.push(' ');
        }
        out.extend(letter.to_lowercase());
    }
    out
}

fn flatten(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip(text: &str) -> String {
    if text.chars().count() <= NAME_LIMIT {
        return text.to_owned();
    }
    let kept: String = text.chars().take(NAME_LIMIT).collect();
    format!("{kept}...")
}
