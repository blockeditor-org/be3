use crate::names::{Modifier, Source};
use toml_edit::{
    Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, TableLike, Value, value,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupOutline {
    pub name: String,
    pub exe: String,
    pub program_id: String,
    pub title: String,
    pub rules: Vec<RuleOutline>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleOutline {
    pub on: String,
    /// What the `held` list names, which must be held as well as what `on` names.
    pub held: Vec<String>,
    pub does: Does,
    pub key: String,
    pub hold_ms: Option<i64>,
    pub not_held: Vec<String>,
    pub keep: bool,
    pub blind: bool,
    pub steps: Vec<StepOutline>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Does {
    #[default]
    Send,
    Tap,
    Steps,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepOutline {
    Tap { key: String, hold_ms: Option<i64> },
    Wait { ms: i64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupField {
    Name,
    Exe,
    ProgramId,
    Title,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleEdit {
    On(String),
    /// Changes the key of `on`, keeping what is held with it.
    Trigger(String),
    /// Adds what must be held as well, or stops requiring it wherever it is
    /// written; a modifier's name stands for either of its keys.
    Held(String, bool),
    Does(Does),
    Key(String),
    HoldMs(Option<i64>),
    /// Adds or removes one of what keeps the rule from applying.
    NotHeld(String, bool),
    Keep(bool),
    Blind(bool),
    AddStep { wait: bool },
    RemoveStep(usize),
    StepKey(usize, String),
    StepMs(usize, Option<i64>),
}

const NEW_KEY: &str = "Space";

pub struct Editing(DocumentMut);

impl Editing {
    pub fn parse(text: &str) -> Result<Self, String> {
        text.parse::<DocumentMut>()
            .map(Self)
            .map_err(|error| error.to_string())
    }

    pub fn text(&self) -> String {
        self.0.to_string()
    }

    pub fn outline(&self) -> Vec<GroupOutline> {
        let Some(groups) = self.0.get("group") else {
            return Vec::new();
        };
        each(groups)
            .into_iter()
            .map(|group| GroupOutline {
                name: text(group.get("name")),
                exe: window(group, "exe"),
                program_id: window(group, "program_id"),
                title: window(group, "title"),
                rules: group
                    .get("rules")
                    .map(each)
                    .unwrap_or_default()
                    .into_iter()
                    .map(rule)
                    .collect(),
            })
            .collect()
    }

    pub fn add_group(&mut self, name: &str) {
        let groups = self
            .0
            .as_table_mut()
            .entry("group")
            .or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
        let mut table = Table::new();
        table.insert("name", value(name));
        table.insert("rules", value(Array::new()));
        match groups {
            Item::ArrayOfTables(groups) => groups.push(table),
            Item::Value(Value::Array(groups)) => {
                push(groups, Value::InlineTable(table.into_inline_table()))
            }
            _ => {}
        }
    }

    pub fn remove_group(&mut self, group: usize) {
        match self.0.get_mut("group") {
            Some(Item::ArrayOfTables(groups)) if group < groups.len() => {
                groups.remove(group);
            }
            Some(Item::Value(Value::Array(groups))) if group < groups.len() => {
                groups.remove(group);
            }
            _ => {}
        }
    }

    pub fn set_group(&mut self, group: usize, field: GroupField, text: &str) {
        let Some(table) = self.group_mut(group) else {
            return;
        };
        let key = match field {
            GroupField::Name => {
                table.insert("name", value(text));
                return;
            }
            GroupField::Exe => "exe",
            GroupField::ProgramId => "program_id",
            GroupField::Title => "title",
        };
        if text.is_empty() {
            if let Some(window) = table.get_mut("window").and_then(Item::as_table_like_mut) {
                window.remove(key);
                if window.is_empty() {
                    table.remove("window");
                }
            }
            return;
        }
        if table.get("window").and_then(Item::as_table_like).is_none() {
            table.insert("window", value(InlineTable::new()));
        }
        if let Some(window) = table.get_mut("window").and_then(Item::as_table_like_mut) {
            window.insert(key, value(text));
        }
    }

    pub fn add_rule(&mut self, group: usize) {
        let Some(table) = self.group_mut(group) else {
            return;
        };
        if table.get("rules").is_none() {
            table.insert("rules", value(Array::new()));
        }
        let mut rule = InlineTable::new();
        rule.insert("on", NEW_KEY.into());
        rule.insert("send", NEW_KEY.into());
        match table.get_mut("rules") {
            Some(Item::Value(Value::Array(rules))) => push(rules, Value::InlineTable(rule)),
            Some(Item::ArrayOfTables(rules)) => rules.push(rule.into_table()),
            _ => {}
        }
    }

    pub fn remove_rule(&mut self, group: usize, rule: usize) {
        let Some(table) = self.group_mut(group) else {
            return;
        };
        match table.get_mut("rules") {
            Some(Item::Value(Value::Array(rules))) if rule < rules.len() => {
                rules.remove(rule);
            }
            Some(Item::ArrayOfTables(rules)) if rule < rules.len() => {
                rules.remove(rule);
            }
            _ => {}
        }
    }

    /// Copies a rule to just after it.
    pub fn duplicate_rule(&mut self, group: usize, rule: usize) {
        let Some(table) = self.group_mut(group) else {
            return;
        };
        match table.get_mut("rules") {
            Some(Item::Value(Value::Array(rules))) if rule < rules.len() => {
                let mut copy = rules.get(rule).cloned().unwrap_or(Value::from(false));
                let prefix = copy.decor().prefix().and_then(|prefix| prefix.as_str()).map(plain);
                copy.decor_mut().set_suffix("");
                if let Some(prefix) = prefix {
                    copy.decor_mut().set_prefix(prefix);
                }
                rules.insert_formatted(rule + 1, copy);
            }
            Some(Item::ArrayOfTables(rules)) if rule < rules.len() => {
                if let Some(copy) = rules.get(rule).cloned() {
                    rules.push(copy);
                }
            }
            _ => {}
        }
    }

    pub fn edit_rule(&mut self, group: usize, rule: usize, edit: RuleEdit) {
        self.change_rule(group, rule, edit);
        if let Some(Item::Value(Value::Array(rules))) =
            self.group_mut(group).and_then(|group| group.get_mut("rules"))
            && let Some(table) = rules.get_mut(rule).and_then(Value::as_inline_table_mut)
        {
            table.fmt();
        }
    }

    fn change_rule(&mut self, group: usize, rule: usize, edit: RuleEdit) {
        let Some(table) = self.rule_mut(group, rule) else {
            return;
        };
        match edit {
            RuleEdit::On(on) => {
                table.insert("on", value(on));
            }
            RuleEdit::Trigger(key) => {
                let mut on = Combo::parse(&text(table.get("on")));
                on.key = key;
                table.insert("on", value(on.text()));
            }
            RuleEdit::Held(name, true) => {
                let mut on = Combo::parse(&text(table.get("on")));
                let listed = table
                    .get("held")
                    .and_then(Item::as_array)
                    .is_some_and(|held| held.iter().filter_map(Value::as_str).any(|held| same(held, &name)));
                if !on.holds(&name) && !listed {
                    on.hold(&name);
                    table.insert("on", value(on.text()));
                }
            }
            RuleEdit::Held(name, false) => {
                let mut on = Combo::parse(&text(table.get("on")));
                if on.holds(&name) {
                    on.release(&name);
                    table.insert("on", value(on.text()));
                }
                if let Some(held) = table.get_mut("held").and_then(Item::as_array_mut) {
                    held.retain(|held| !held.as_str().is_some_and(|held| same(held, &name)));
                    if held.is_empty() {
                        table.remove("held");
                    }
                }
            }
            RuleEdit::Does(does) => {
                let key = action_key(table);
                for old in ["send", "tap", "hold_ms", "steps"] {
                    table.remove(old);
                }
                match does {
                    Does::Send => {
                        table.insert("send", value(key));
                    }
                    Does::Tap => {
                        table.insert("tap", value(key));
                    }
                    Does::Steps => {
                        let mut step = InlineTable::new();
                        step.insert("tap", key.into());
                        let mut steps = Array::new();
                        steps.push(step);
                        table.insert("steps", value(steps));
                    }
                }
            }
            RuleEdit::Key(key) => {
                let field = if table.contains_key("tap") {
                    "tap"
                } else {
                    "send"
                };
                table.insert(field, value(key));
            }
            RuleEdit::HoldMs(ms) => set_ms(table, "hold_ms", ms),
            RuleEdit::NotHeld(name, on) => {
                if table.get("not_held").and_then(Item::as_array).is_none() {
                    table.insert("not_held", value(Array::new()));
                }
                if let Some(names) = table.get_mut("not_held").and_then(Item::as_array_mut) {
                    let listed = names
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|listed| same(listed, &name));
                    match on {
                        true if !listed => names.push(name.as_str()),
                        true => {}
                        false => names.retain(|listed| {
                            !listed.as_str().is_some_and(|listed| same(listed, &name))
                        }),
                    }
                    if names.is_empty() {
                        table.remove("not_held");
                    }
                }
            }
            RuleEdit::Keep(on) => set_flag(table, "keep", on),
            RuleEdit::Blind(on) => set_flag(table, "blind", on),
            RuleEdit::AddStep { wait } => {
                let mut step = InlineTable::new();
                match wait {
                    true => step.insert("wait_ms", 10.into()),
                    false => step.insert("tap", NEW_KEY.into()),
                };
                if let Some(steps) = table.get_mut("steps").and_then(Item::as_array_mut) {
                    push(steps, Value::InlineTable(step));
                }
            }
            RuleEdit::RemoveStep(step) => {
                if let Some(steps) = table.get_mut("steps").and_then(Item::as_array_mut)
                    && step < steps.len()
                {
                    let removed = steps.remove(step);
                    if step == 0
                        && let Some(first) = steps.get_mut(0)
                    {
                        let prefix = removed.decor().prefix().cloned().unwrap_or_default();
                        first.decor_mut().set_prefix(prefix);
                    }
                }
            }
            RuleEdit::StepKey(step, key) => {
                if let Some(step) = step_mut(table, step) {
                    step.insert("tap", key.into());
                }
            }
            RuleEdit::StepMs(step, ms) => {
                if let Some(step) = step_mut(table, step) {
                    let field = if step.contains_key("wait_ms") {
                        "wait_ms"
                    } else {
                        "hold_ms"
                    };
                    match (field, ms) {
                        ("wait_ms", None) => {
                            step.insert("wait_ms", 0.into());
                        }
                        (field, ms) => set_ms(step, field, ms),
                    }
                }
            }
        }
    }

    fn group_mut(&mut self, group: usize) -> Option<&mut dyn TableLike> {
        match self.0.get_mut("group")? {
            Item::ArrayOfTables(groups) => groups
                .get_mut(group)
                .map(|table| table as &mut dyn TableLike),
            Item::Value(Value::Array(groups)) => groups
                .get_mut(group)
                .and_then(Value::as_inline_table_mut)
                .map(|table| table as &mut dyn TableLike),
            _ => None,
        }
    }

    fn rule_mut(&mut self, group: usize, rule: usize) -> Option<&mut dyn TableLike> {
        match self.group_mut(group)?.get_mut("rules")? {
            Item::ArrayOfTables(rules) => {
                rules.get_mut(rule).map(|table| table as &mut dyn TableLike)
            }
            Item::Value(Value::Array(rules)) => rules
                .get_mut(rule)
                .and_then(Value::as_inline_table_mut)
                .map(|table| table as &mut dyn TableLike),
            _ => None,
        }
    }
}

fn each(item: &Item) -> Vec<&dyn TableLike> {
    match item {
        Item::ArrayOfTables(tables) => tables.iter().map(|table| table as &dyn TableLike).collect(),
        Item::Value(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_inline_table)
            .map(|table| table as &dyn TableLike)
            .collect(),
        _ => Vec::new(),
    }
}

fn text(item: Option<&Item>) -> String {
    item.and_then(Item::as_str).unwrap_or_default().to_owned()
}

fn window(group: &dyn TableLike, key: &str) -> String {
    text(
        group
            .get("window")
            .and_then(Item::as_table_like)
            .and_then(|window| window.get(key)),
    )
}

fn rule(table: &dyn TableLike) -> RuleOutline {
    let does = if table.contains_key("steps") {
        Does::Steps
    } else if table.contains_key("tap") {
        Does::Tap
    } else {
        Does::Send
    };
    let steps = table
        .get("steps")
        .and_then(Item::as_array)
        .map(|steps| {
            steps
                .iter()
                .filter_map(Value::as_inline_table)
                .map(
                    |step| match step.get("wait_ms").and_then(Value::as_integer) {
                        Some(ms) => StepOutline::Wait { ms },
                        None => StepOutline::Tap {
                            key: step
                                .get("tap")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                            hold_ms: step.get("hold_ms").and_then(Value::as_integer),
                        },
                    },
                )
                .collect()
        })
        .unwrap_or_default();
    let names = |key: &str| {
        table
            .get(key)
            .and_then(Item::as_array)
            .map(|names| names.iter().filter_map(Value::as_str).map(str::to_owned).collect())
            .unwrap_or_default()
    };
    RuleOutline {
        on: text(table.get("on")),
        held: names("held"),
        does,
        key: action_key(table),
        hold_ms: table.get("hold_ms").and_then(Item::as_integer),
        not_held: names("not_held"),
        keep: table.get("keep").and_then(Item::as_bool).unwrap_or(false),
        blind: table.get("blind").and_then(Item::as_bool).unwrap_or(false),
        steps,
    }
}

fn action_key(table: &dyn TableLike) -> String {
    let direct = ["send", "tap"]
        .into_iter()
        .find_map(|key| table.get(key).and_then(Item::as_str));
    let first_step = || {
        table
            .get("steps")
            .and_then(Item::as_array)
            .and_then(|steps| {
                steps
                    .iter()
                    .find_map(|step| step.as_inline_table()?.get("tap")?.as_str())
            })
    };
    direct.or_else(first_step).unwrap_or(NEW_KEY).to_owned()
}

fn set_ms(table: &mut dyn TableLike, key: &str, ms: Option<i64>) {
    match ms {
        Some(ms) if ms > 0 => {
            table.insert(key, value(ms));
        }
        _ => {
            table.remove(key);
        }
    }
}

fn set_flag(table: &mut dyn TableLike, key: &str, on: bool) {
    match on {
        true => {
            table.insert(key, value(true));
        }
        false => {
            table.remove(key);
        }
    }
}

fn step_mut(table: &mut dyn TableLike, step: usize) -> Option<&mut InlineTable> {
    table
        .get_mut("steps")?
        .as_array_mut()?
        .get_mut(step)?
        .as_inline_table_mut()
}

fn push(array: &mut Array, mut item: Value) {
    let prefix = array
        .iter()
        .last()
        .and_then(|last| last.decor().prefix()?.as_str().map(plain));
    match prefix {
        Some(prefix) if !prefix.is_empty() => item.decor_mut().set_prefix(prefix),
        Some(_) => item.decor_mut().set_prefix(" "),
        None => {}
    }
    array.push_formatted(item);
}

/// A prefix without the comments it carries, which belong to the line before.
fn plain(prefix: &str) -> String {
    match prefix.rfind('\n') {
        Some(line) => prefix[line..].to_owned(),
        None if prefix.contains('#') => " ".to_owned(),
        None => prefix.to_owned(),
    }
}

/// A key written with what is held with it, `Ctrl+Shift+W` or `F20+WheelUp`,
/// as its parts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Combo {
    pub held: Vec<String>,
    pub key: String,
}

impl Combo {
    pub fn parse(text: &str) -> Self {
        let mut held: Vec<String> = text
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_owned)
            .collect();
        let key = held.pop().unwrap_or_default();
        Self { held, key }
    }

    pub fn text(&self) -> String {
        self.held
            .iter()
            .chain(std::iter::once(&self.key))
            .cloned()
            .collect::<Vec<_>>()
            .join("+")
    }

    pub fn holds(&self, name: &str) -> bool {
        self.held.iter().any(|held| same(held, name))
    }

    /// Holds a name too, modifiers first in the order Shift, Ctrl, Alt, Super.
    pub fn hold(&mut self, name: &str) {
        if self.holds(name) {
            return;
        }
        let at = match Modifier::named(name) {
            Some(modifier) => self
                .held
                .iter()
                .position(|held| modifier_of(held).is_none_or(|held| held > modifier))
                .unwrap_or(self.held.len()),
            None => self.held.len(),
        };
        self.held.insert(at, name.to_owned());
    }

    pub fn release(&mut self, name: &str) {
        self.held.retain(|held| !same(held, name));
    }

    /// The modifiers held, whichever side's key names them.
    pub fn modifiers(&self) -> Vec<Modifier> {
        self.held.iter().filter_map(|held| modifier_of(held)).collect()
    }

    /// What is held that is no modifier, like the F20 of `F20+WheelUp`.
    pub fn others(&self) -> Vec<String> {
        self.held
            .iter()
            .filter(|held| modifier_of(held).is_none())
            .cloned()
            .collect()
    }
}

/// The modifier a name stands for: `Shift`, or either of its keys.
pub fn modifier_of(name: &str) -> Option<Modifier> {
    Modifier::named(name.trim()).or_else(|| Source::named(name).and_then(Modifier::of))
}

/// Whether two names mean the same: a modifier's name matches either of its
/// keys, and aliases match what they stand for.
fn same(written: &str, name: &str) -> bool {
    if let Some(modifier) = Modifier::named(name.trim()) {
        return modifier_of(written) == Some(modifier);
    }
    match (Source::named(written), Source::named(name)) {
        (Some(written), Some(name)) => written == name,
        _ => written.trim().eq_ignore_ascii_case(name.trim()),
    }
}
