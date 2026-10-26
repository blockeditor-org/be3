use std::collections::HashMap;

use block_plugin_api::{
    HostImage, HostTrayItem, MAX_MENU_DEPTH, TrayMenuItem, TrayStatus, TrayToggle,
};
use image::imageops::FilterType;
use zbus::zvariant::{OwnedValue, Value};

pub(super) const ITEM_PATH: &str = "/StatusNotifierItem";
pub(super) const MAX_ITEMS: usize = 64;
const MAX_ENTRIES: usize = 256;
const MAX_TEXT: usize = 1024;
const MAX_PIXMAP_SIDE: i32 = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Address {
    pub(super) bus: String,
    pub(super) path: String,
}

impl Address {
    pub(super) fn registered(service: &str, sender: Option<&str>) -> Option<Self> {
        match service.starts_with('/') {
            true => Some(Self {
                bus: sender.filter(|sender| !sender.is_empty())?.to_owned(),
                path: service.to_owned(),
            }),
            false => Self::listed(service),
        }
    }

    pub(super) fn listed(service: &str) -> Option<Self> {
        let (bus, path) = match service.find('/') {
            Some(slash) => (&service[..slash], service[slash..].to_owned()),
            None => (service, ITEM_PATH.to_owned()),
        };
        (!bus.is_empty()).then(|| Self {
            bus: bus.to_owned(),
            path,
        })
    }

    pub(super) fn key(&self) -> String {
        format!("{}{}", self.bus, self.path)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Pixmap {
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) argb: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Properties {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) tooltip: String,
    pub(super) status: TrayStatus,
    pub(super) icon_name: String,
    pub(super) icon_pixmaps: Vec<Pixmap>,
    pub(super) attention_icon_name: String,
    pub(super) attention_pixmaps: Vec<Pixmap>,
    pub(super) theme_path: String,
    pub(super) item_is_menu: bool,
    pub(super) menu: Option<String>,
}

fn inner<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
    match value {
        Value::Value(boxed) => inner(boxed),
        value => value,
    }
}

fn text(value: Option<&Value<'_>>) -> String {
    match value.map(inner) {
        Some(Value::Str(text)) => clip(text.as_str()),
        Some(Value::ObjectPath(path)) => clip(path.as_str()),
        _ => String::new(),
    }
}

fn flag(value: Option<&Value<'_>>) -> Option<bool> {
    match value.map(inner) {
        Some(Value::Bool(flag)) => Some(*flag),
        _ => None,
    }
}

fn number(value: Option<&Value<'_>>) -> Option<i32> {
    match value.map(inner) {
        Some(Value::I32(number)) => Some(*number),
        _ => None,
    }
}

pub(super) fn clip(text: &str) -> String {
    if text.len() <= MAX_TEXT {
        return text.to_owned();
    }
    let mut end = MAX_TEXT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn pixmaps(value: Option<&Value<'_>>) -> Vec<Pixmap> {
    let Some(Value::Array(array)) = value.map(inner) else {
        return Vec::new();
    };
    array
        .inner()
        .iter()
        .filter_map(|pixmap| {
            let Value::Structure(pixmap) = inner(pixmap) else {
                return None;
            };
            let [Value::I32(width), Value::I32(height), Value::Array(bytes)] = pixmap.fields()
            else {
                return None;
            };
            if !(1..=MAX_PIXMAP_SIDE).contains(width) || !(1..=MAX_PIXMAP_SIDE).contains(height) {
                return None;
            }
            let argb: Vec<u8> = bytes
                .inner()
                .iter()
                .map(|byte| match byte {
                    Value::U8(byte) => Some(*byte),
                    _ => None,
                })
                .collect::<Option<_>>()?;
            Some(Pixmap {
                width: *width,
                height: *height,
                argb,
            })
        })
        .collect()
}

fn tooltip(value: Option<&Value<'_>>) -> String {
    let Some(Value::Structure(tooltip)) = value.map(inner) else {
        return String::new();
    };
    let [_, _, Value::Str(title), Value::Str(description)] = tooltip.fields() else {
        return String::new();
    };
    match title.as_str().trim() {
        "" => clip(strip_markup(description.as_str()).trim()),
        title => clip(title),
    }
}

fn strip_markup(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    let mut in_tag = false;
    for character in text.chars() {
        match (in_tag, character) {
            (false, '<') => in_tag = true,
            (true, '>') => in_tag = false,
            (false, character) => plain.push(character),
            (true, _) => {}
        }
    }
    plain
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

pub(super) fn properties(map: &HashMap<String, OwnedValue>) -> Properties {
    let get = |key: &str| map.get(key).map(|value| &**value);
    let status = match text(get("Status")).as_str() {
        "Passive" => TrayStatus::Passive,
        "NeedsAttention" => TrayStatus::NeedsAttention,
        _ => TrayStatus::Active,
    };
    let menu = text(get("Menu"));
    Properties {
        id: text(get("Id")),
        title: text(get("Title")),
        tooltip: tooltip(get("ToolTip")),
        status,
        icon_name: text(get("IconName")),
        icon_pixmaps: pixmaps(get("IconPixmap")),
        attention_icon_name: text(get("AttentionIconName")),
        attention_pixmaps: pixmaps(get("AttentionIconPixmap")),
        theme_path: text(get("IconThemePath")),
        item_is_menu: flag(get("ItemIsMenu")).unwrap_or(false),
        menu: (!menu.is_empty() && menu != "/").then_some(menu),
    }
}

pub(super) fn pixmap_image(pixmaps: &[Pixmap], pixels: u32) -> Option<HostImage> {
    let whole = pixmaps
        .iter()
        .filter(|pixmap| pixmap.argb.len() == pixmap.width as usize * pixmap.height as usize * 4);
    let side = |pixmap: &Pixmap| pixmap.width.max(pixmap.height) as u32;
    let chosen = whole
        .clone()
        .filter(|pixmap| side(pixmap) >= pixels)
        .min_by_key(|pixmap| side(pixmap))
        .or_else(|| whole.max_by_key(|pixmap| side(pixmap)))?;
    let rgba: Vec<u8> = chosen
        .argb
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|argb| [argb[1], argb[2], argb[3], argb[0]])
        .collect();
    let (width, height) = (chosen.width as u32, chosen.height as u32);
    Some(shrink(width, height, rgba, pixels))
}

pub(super) fn shrink(width: u32, height: u32, rgba: Vec<u8>, pixels: u32) -> HostImage {
    if width.max(height) <= pixels {
        return HostImage {
            width,
            height,
            rgba,
        };
    }
    let Some(full) = image::RgbaImage::from_raw(width, height, rgba) else {
        return HostImage::default();
    };
    let scale = pixels as f32 / width.max(height) as f32;
    let side = |length: u32| ((length as f32 * scale).round() as u32).max(1);
    let shrunk = image::imageops::resize(&full, side(width), side(height), FilterType::Triangle);
    HostImage {
        width: shrunk.width(),
        height: shrunk.height(),
        rgba: shrunk.into_raw(),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct RawEntry {
    pub(super) id: i32,
    pub(super) kind: String,
    pub(super) label: String,
    pub(super) enabled: Option<bool>,
    pub(super) visible: Option<bool>,
    pub(super) toggle_type: String,
    pub(super) toggle_state: Option<i32>,
    pub(super) children: Vec<RawEntry>,
}

pub(super) fn raw_entry<'a>(
    id: i32,
    properties: impl Iterator<Item = (&'a str, &'a Value<'a>)>,
    children: impl Iterator<Item = &'a Value<'a>>,
    depth: usize,
) -> RawEntry {
    let mut entry = RawEntry {
        id,
        ..RawEntry::default()
    };
    for (key, value) in properties {
        match key {
            "type" => entry.kind = text(Some(value)),
            "label" => entry.label = text(Some(value)),
            "enabled" => entry.enabled = flag(Some(value)),
            "visible" => entry.visible = flag(Some(value)),
            "toggle-type" => entry.toggle_type = text(Some(value)),
            "toggle-state" => entry.toggle_state = number(Some(value)),
            _ => {}
        }
    }
    if depth < MAX_MENU_DEPTH {
        entry.children = children
            .take(MAX_ENTRIES)
            .filter_map(|child| raw_child(child, depth + 1))
            .collect();
    }
    entry
}

fn raw_child<'a>(value: &'a Value<'a>, depth: usize) -> Option<RawEntry> {
    let Value::Structure(structure) = inner(value) else {
        return None;
    };
    let [
        Value::I32(id),
        Value::Dict(properties),
        Value::Array(children),
    ] = structure.fields()
    else {
        return None;
    };
    let properties = properties
        .iter()
        .filter_map(|(key, value)| match inner(key) {
            Value::Str(key) => Some((key.as_str(), value)),
            _ => None,
        });
    Some(raw_entry(*id, properties, children.inner().iter(), depth))
}

pub(super) fn menu(root: &RawEntry) -> Vec<TrayMenuItem> {
    let mut items = Vec::new();
    flatten(root, 0, &mut items);
    items
}

fn flatten(parent: &RawEntry, depth: u32, items: &mut Vec<TrayMenuItem>) {
    for raw in &parent.children {
        let Some(item) = entry(raw, depth) else {
            continue;
        };
        let separator = item.separator;
        items.push(item);
        if !separator {
            flatten(raw, depth + 1, items);
        }
    }
}

fn entry(raw: &RawEntry, depth: u32) -> Option<TrayMenuItem> {
    if raw.visible == Some(false) {
        return None;
    }
    let separator = raw.kind == "separator";
    let on = raw.toggle_state == Some(1);
    Some(TrayMenuItem {
        depth,
        id: raw.id,
        label: match separator {
            true => String::new(),
            false => mnemonic_free(&raw.label),
        },
        separator,
        enabled: raw.enabled.unwrap_or(true),
        toggle: match raw.toggle_type.as_str() {
            "checkmark" => TrayToggle::Check(on),
            "radio" => TrayToggle::Radio(on),
            _ => TrayToggle::None,
        },
    })
}

pub(super) fn mnemonic_free(label: &str) -> String {
    let mut plain = String::with_capacity(label.len());
    let mut characters = label.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '_' {
            plain.push(character);
            continue;
        }
        if characters.peek() == Some(&'_') {
            characters.next();
            plain.push('_');
        }
    }
    plain
}

#[derive(Debug, Default)]
pub(super) struct Item {
    pub(super) address: Address,
    pub(super) owner: String,
    pub(super) properties: Option<Properties>,
    pub(super) icon: Option<HostImage>,
    pub(super) menu: Vec<TrayMenuItem>,
    pub(super) asked: u64,
    pub(super) menu_asked: u64,
}

impl Default for Address {
    fn default() -> Self {
        Self {
            bus: String::new(),
            path: ITEM_PATH.to_owned(),
        }
    }
}

impl Item {
    pub(super) fn menu_path(&self) -> Option<&str> {
        self.properties.as_ref()?.menu.as_deref()
    }

    fn published(&self) -> Option<HostTrayItem> {
        let properties = self.properties.as_ref()?;
        let title = [&properties.title, &properties.tooltip, &properties.id]
            .into_iter()
            .find(|title| !title.trim().is_empty())
            .cloned()
            .unwrap_or_default();
        Some(HostTrayItem {
            id: self.address.key(),
            title,
            tooltip: properties.tooltip.clone(),
            status: properties.status,
            icon: self.icon.clone(),
            item_is_menu: properties.item_is_menu,
            menu: self.menu.clone(),
        })
    }
}

#[derive(Debug, Default)]
pub(super) struct Items {
    items: Vec<Item>,
}

impl Items {
    pub(super) fn add(&mut self, address: Address, owner: String) -> bool {
        if let Some(item) = self.get_mut(&address.key()) {
            item.owner = owner;
            return false;
        }
        if self.items.len() >= MAX_ITEMS {
            return false;
        }
        self.items.push(Item {
            address,
            owner,
            ..Item::default()
        });
        true
    }

    pub(super) fn get(&self, key: &str) -> Option<&Item> {
        self.items.iter().find(|item| item.address.key() == key)
    }

    pub(super) fn get_mut(&mut self, key: &str) -> Option<&mut Item> {
        self.items.iter_mut().find(|item| item.address.key() == key)
    }

    pub(super) fn remove(&mut self, key: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.address.key() != key);
        self.items.len() != before
    }

    pub(super) fn vanished(&mut self, name: &str) -> Vec<String> {
        let mut gone = Vec::new();
        self.items.retain(|item| {
            let kept = item.address.bus != name && item.owner != name;
            if !kept {
                gone.push(item.address.key());
            }
            kept
        });
        gone
    }

    pub(super) fn signalled(&self, sender: &str, path: &str) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| item.owner == sender && item.address.path == path)
            .map(|item| item.address.key())
            .collect()
    }

    pub(super) fn menu_signalled(&self, sender: &str, path: &str) -> Vec<String> {
        self.items
            .iter()
            .filter(|item| item.owner == sender && item.menu_path() == Some(path))
            .map(|item| item.address.key())
            .collect()
    }

    pub(super) fn published(&self) -> Vec<HostTrayItem> {
        self.items.iter().filter_map(Item::published).collect()
    }
}
