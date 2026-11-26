//! `schema.lock`: the ids of a crate's `Schema` types (be-wasm-schema).
//!
//! The source says what a type is and whether it is frozen: every `Schema` type is marked
//! `#[schema(frozen)]` (no breaking changes) or `#[schema(unfrozen)]` (change it however it
//! needs). The lockfile says which id each field and variant has, so the source may reorder
//! them freely. ./scripts/verify rewrites the lockfile from the source and the lockfile before it
//! (`update`), and the derive numbers a type from the lockfile checked in (`numbered`) the same
//! way, so a build never waits for the lockfile to catch up. That a frozen type holds only frozen
//! types is the compiler's to check (be-wasm-schema's `Encode::FROZEN`), across crates.
//!
//! Every entry is updated incrementally, frozen or not: a member keeps its id while its name
//! stays, a new one takes the next unused id, and a removed one stays as `retired`, with its
//! type, so its id is never given again. An unfrozen entry accepts every change. A frozen one
//! refuses what an older build could not read:
//!
//! - a member retyped, or given or relieved of `critical`, `no_copy` or a default (a member's
//!   shape is its type and those, as written);
//! - a frozen type marked unfrozen again, or removed;
//! - a frozen enum that says neither whether it keeps unknown variants (a `#[schema(unknown)]`
//!   case) nor that it never will (`#[schema(closed)]`); a variant added to a closed one; an
//!   open one closed; a struct that kept unknown fields no longer keeping them;
//! - a retired name given again.
//!
//! A frozen member is removed or renamed only with a hand edit here, so neither happens by
//! accident: removing one means marking it `retired` (an API that forgot a field that meant
//! something would answer wrongly rather than fail), and renaming one means renaming it.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;

/// The lockfile's name, beside the crate's `Cargo.toml`.
pub const FILE: &str = "schema.lock";

const HEADER: &str = "\
# Written by ./scripts/verify from this crate's Schema types (be-wasm-schema): the id of each
# field and variant. Rename a member of a frozen type here when you rename it in the source, and
# confirm a frozen member's removal by writing `retired` before its name.
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Struct,
    Enum,
    /// A struct with one unnamed field, encoded as that field.
    Newtype,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Struct => "struct",
            Kind::Enum => "enum",
            Kind::Newtype => "newtype",
        }
    }
}

/// A member of a struct (a field) or an enum (a variant), as the lockfile keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub id: u32,
    pub name: String,
    /// `: Type` for a field, empty for a unit variant, `(Type)` for a newtype variant and `{}`
    /// for a struct variant (whose fields are the entry `Enum::Variant`), then any of
    /// ` [critical, no_copy, default = ...]`. A retired member keeps the shape it had.
    pub shape: String,
    pub retired: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    /// A newtype's field type.
    pub inner: String,
    /// Whether an enum keeps variants it does not know (a `#[schema(unknown)]` case).
    pub open: bool,
    /// Whether an enum says it never will (`#[schema(closed)]`).
    pub closed: bool,
    /// Whether a struct keeps fields it does not know (a `#[schema(unknown)]` field).
    pub keeps_unknown: bool,
    /// Set once frozen: the first id given after freezing. A member with this id or above was
    /// added to a frozen type, so it must decode when missing.
    pub frozen_from: Option<u32>,
    /// By id, retired ones included.
    pub members: Vec<Member>,
}

impl Entry {
    pub fn id(&self, name: &str) -> Option<u32> {
        self.members
            .iter()
            .find(|member| !member.retired && member.name == name)
            .map(|member| member.id)
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen_from.is_some()
    }

    fn next(&self) -> u32 {
        self.members
            .iter()
            .map(|member| member.id)
            .max()
            .unwrap_or(0)
            + 1
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lock {
    /// By type name; a struct variant's fields are `Enum::Variant`.
    pub types: BTreeMap<String, Entry>,
}

impl Lock {
    pub fn parse(text: &str) -> Result<Lock, String> {
        let mut lock = Lock::default();
        let mut current: Option<(String, Entry)> = None;
        for (number, line) in text.lines().enumerate() {
            let fail = |what: &str| format!("{FILE}:{}: {what}: {line}", number + 1);
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(member) = line.strip_prefix("  ") {
                let Some((_, entry)) = current.as_mut() else {
                    return Err(fail("a member outside a type"));
                };
                match member {
                    "open" => entry.open = true,
                    "closed" => entry.closed = true,
                    "keeps unknown" => entry.keeps_unknown = true,
                    _ => {
                        if let Some(from) = member.strip_prefix("frozen from ") {
                            entry.frozen_from = Some(from.parse().map_err(|_| fail("not an id"))?);
                            continue;
                        }
                        let (id, rest) = member.split_once(' ').ok_or_else(|| fail("no id"))?;
                        let id: u32 = id.parse().map_err(|_| fail("not an id"))?;
                        let (retired, rest) = match rest.strip_prefix("retired ") {
                            Some(rest) => (true, rest),
                            None => (false, rest),
                        };
                        let end = rest
                            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '#'))
                            .unwrap_or(rest.len());
                        let (name, shape) = rest.split_at(end);
                        if id == 0 || entry.members.iter().any(|member| member.id == id) {
                            return Err(fail("an id given twice, or zero"));
                        }
                        entry.members.push(Member {
                            id,
                            name: name.to_owned(),
                            shape: shape.to_owned(),
                            retired,
                        });
                    }
                }
                continue;
            }
            if let Some((name, entry)) = current.take() {
                lock.insert(name, entry);
            }
            let (word, rest) = line.split_once(' ').ok_or_else(|| fail("not a type"))?;
            let kind = match word {
                "struct" => Kind::Struct,
                "enum" => Kind::Enum,
                "newtype" => Kind::Newtype,
                _ => return Err(fail("not a type")),
            };
            let (name, inner) = match kind {
                Kind::Newtype => rest
                    .split_once(": ")
                    .ok_or_else(|| fail("a newtype's type"))?,
                _ => (rest, ""),
            };
            let entry = Entry {
                kind,
                inner: inner.to_owned(),
                open: false,
                closed: false,
                keeps_unknown: false,
                frozen_from: None,
                members: Vec::new(),
            };
            current = Some((name.to_owned(), entry));
        }
        if let Some((name, entry)) = current.take() {
            lock.insert(name, entry);
        }
        Ok(lock)
    }

    fn insert(&mut self, name: String, mut entry: Entry) {
        entry.members.sort_by_key(|member| member.id);
        self.types.insert(name, entry);
    }

    pub fn to_text(&self) -> String {
        let mut text = HEADER.to_owned();
        for (name, entry) in &self.types {
            text.push('\n');
            let _ = write!(text, "{} {name}", entry.kind.word());
            if entry.kind == Kind::Newtype {
                let _ = write!(text, ": {}", entry.inner);
            }
            text.push('\n');
            for (flag, set) in [
                ("open", entry.open),
                ("closed", entry.closed),
                ("keeps unknown", entry.keeps_unknown),
            ] {
                if set {
                    let _ = writeln!(text, "  {flag}");
                }
            }
            if let Some(from) = entry.frozen_from {
                let _ = writeln!(text, "  frozen from {from}");
            }
            for member in &entry.members {
                let retired = if member.retired { "retired " } else { "" };
                let _ = writeln!(
                    text,
                    "  {} {retired}{}{}",
                    member.id, member.name, member.shape
                );
            }
        }
        text
    }
}

/// A `Schema` type as its source describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Described {
    pub name: String,
    pub kind: Kind,
    pub inner: String,
    /// `#[schema(frozen)]`, or a struct variant of a frozen enum.
    pub frozen: bool,
    pub open: bool,
    pub closed: bool,
    pub keeps_unknown: bool,
    /// By name, with their shapes (`Member::shape`), in source order.
    pub members: Vec<(String, String)>,
}

/// Whether `attributes` hold `#[schema(word)]`.
pub fn has_schema_word(attributes: &[syn::Attribute], word: &str) -> bool {
    let mut found = false;
    for attribute in attributes {
        if attribute.path().is_ident("schema") {
            let _ = attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident(word) {
                    found = true;
                }
                if meta.input.peek(syn::Token![=]) {
                    let _: syn::Expr = meta.value()?.parse()?;
                }
                Ok(())
            });
        }
    }
    found
}

/// The expression of `#[schema(default = ...)]`, if `attributes` hold one.
pub fn schema_default(attributes: &[syn::Attribute]) -> Option<syn::Expr> {
    let mut found = None;
    for attribute in attributes {
        if attribute.path().is_ident("schema") {
            let _ = attribute.parse_nested_meta(|meta| {
                if meta.input.peek(syn::Token![=]) {
                    let value: syn::Expr = meta.value()?.parse()?;
                    if meta.path.is_ident("default") {
                        found = Some(value);
                    }
                }
                Ok(())
            });
        }
    }
    found
}

/// Whether `attributes` derive `Schema` (by any path ending in it).
pub fn derives_schema(attributes: &[syn::Attribute]) -> bool {
    let mut found = false;
    for attribute in attributes {
        if attribute.path().is_ident("derive") {
            let _ = attribute.parse_nested_meta(|meta| {
                if meta
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "Schema")
                {
                    found = true;
                }
                Ok(())
            });
        }
    }
    found
}

/// Tokens as the lockfile writes them: spaces only between words, after commas and
/// semicolons, and before a bracket that follows a word.
pub fn tokens_text(tokens: TokenStream) -> String {
    fn walk(tokens: TokenStream, out: &mut String, word: &mut bool) {
        for token in tokens {
            match token {
                TokenTree::Ident(ident) => {
                    if *word {
                        out.push(' ');
                    }
                    out.push_str(&ident.to_string());
                    *word = true;
                }
                TokenTree::Literal(literal) => {
                    if *word {
                        out.push(' ');
                    }
                    out.push_str(&literal.to_string());
                    *word = true;
                }
                TokenTree::Punct(punct) => {
                    out.push(punct.as_char());
                    if matches!(punct.as_char(), ',' | ';') {
                        out.push(' ');
                    }
                    *word = false;
                }
                TokenTree::Group(group) => {
                    let (open, close) = match group.delimiter() {
                        Delimiter::Parenthesis => ("(", ")"),
                        Delimiter::Bracket => ("[", "]"),
                        Delimiter::Brace => ("{", "}"),
                        Delimiter::None => ("", ""),
                    };
                    if *word && group.delimiter() == Delimiter::Bracket {
                        out.push(' ');
                    }
                    out.push_str(open);
                    *word = false;
                    walk(group.stream(), out, word);
                    out.push_str(close);
                    *word = false;
                }
            }
        }
    }
    let mut out = String::new();
    walk(tokens, &mut out, &mut false);
    out
}

/// A type as the lockfile writes it.
pub fn shape_of(ty: &syn::Type) -> String {
    tokens_text(ty.to_token_stream())
}

/// A member's flags as the lockfile writes them after its shape.
fn flags_of(attributes: &[syn::Attribute]) -> String {
    let mut flags = Vec::new();
    if has_schema_word(attributes, "critical") {
        flags.push("critical".to_owned());
    }
    if has_schema_word(attributes, "no_copy") {
        flags.push("no_copy".to_owned());
    }
    if let Some(default) = schema_default(attributes) {
        flags.push(format!(
            "default = {}",
            tokens_text(default.to_token_stream())
        ));
    }
    if flags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", flags.join(", "))
    }
}

/// What `input` is, refused where the format cannot carry it: a type marked neither frozen nor
/// unfrozen (or both), with const generics or more than one lifetime, a tuple struct or variant
/// of more than one field, more than one unknown case or field, a default case that carries
/// data, or a pointer-sized integer. A struct variant adds an entry of its own,
/// `Enum::Variant`, after its enum.
pub fn describe(input: &syn::DeriveInput) -> Result<Vec<Described>, String> {
    let name = input.ident.to_string();
    let frozen = has_schema_word(&input.attrs, "frozen");
    if frozen == has_schema_word(&input.attrs, "unfrozen") {
        return Err(format!(
            "{name}: a Schema type says whether it is #[schema(frozen)] (no breaking changes: older builds keep reading it) or #[schema(unfrozen)] (change it however it needs)"
        ));
    }
    let generics = &input.generics.params;
    let lifetimes = input.generics.lifetimes().count();
    if lifetimes > 1
        || generics
            .iter()
            .any(|param| matches!(param, syn::GenericParam::Const(_)))
    {
        return Err(format!(
            "{name}: a Schema type takes at most one lifetime, and no const parameters"
        ));
    }
    let portable = |ty: &syn::Type, of: &str| -> Result<String, String> {
        let shape = shape_of(ty);
        let mut words = shape.split(|c: char| !(c.is_alphanumeric() || c == '_'));
        if let Some(word) = words.find(|word| matches!(*word, "usize" | "isize")) {
            return Err(format!(
                "{of}: {word} is as wide as a pointer, which differs between wasm32 and 64-bit hosts; use a fixed-size integer (u32, u64)"
            ));
        }
        Ok(shape)
    };
    let fields =
        |fields: &syn::FieldsNamed, of: &str| -> Result<(Vec<(String, String)>, bool), String> {
            let mut members = Vec::new();
            let mut unknown = 0;
            for field in &fields.named {
                if has_schema_word(&field.attrs, "unknown") {
                    unknown += 1;
                    continue;
                }
                let ident = field
                    .ident
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                let shape = portable(&field.ty, &format!("{of}.{ident}"))?;
                members.push((ident, format!(": {shape}{}", flags_of(&field.attrs))));
            }
            if unknown > 1 {
                return Err(format!("{of}: at most one field keeps unknown fields"));
            }
            Ok((members, unknown == 1))
        };
    let plain = |name: String, kind, members, keeps_unknown| Described {
        name,
        kind,
        inner: String::new(),
        frozen,
        open: false,
        closed: false,
        keeps_unknown,
        members,
    };
    match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(named) => {
                let (members, keeps_unknown) = fields(named, &name)?;
                Ok(vec![plain(name, Kind::Struct, members, keeps_unknown)])
            }
            syn::Fields::Unit => Ok(vec![plain(name, Kind::Struct, Vec::new(), false)]),
            syn::Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                let inner = portable(&unnamed.unnamed[0].ty, &name)?;
                let mut described = plain(name, Kind::Newtype, Vec::new(), false);
                described.inner = inner;
                Ok(vec![described])
            }
            syn::Fields::Unnamed(_) => Err(format!(
                "{name}: a tuple struct of more than one field has no names to number; name its fields"
            )),
        },
        syn::Data::Enum(data) => {
            let mut members = Vec::new();
            let mut nested = Vec::new();
            let mut unknown = 0;
            let mut defaults = 0;
            for variant in &data.variants {
                let variant_name = variant.ident.to_string();
                if has_schema_word(&variant.attrs, "default") {
                    defaults += 1;
                    if !matches!(variant.fields, syn::Fields::Unit) {
                        return Err(format!(
                            "{name}::{variant_name}: only a unit variant can be the default"
                        ));
                    }
                }
                if has_schema_word(&variant.attrs, "unknown") {
                    unknown += 1;
                    continue;
                }
                let shape = match &variant.fields {
                    syn::Fields::Unit => String::new(),
                    syn::Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => {
                        let of = format!("{name}::{variant_name}");
                        format!("({})", portable(&unnamed.unnamed[0].ty, &of)?)
                    }
                    syn::Fields::Unnamed(_) => {
                        return Err(format!(
                            "{name}::{variant_name}: a variant of more than one unnamed field has no names to number; make it a struct variant"
                        ));
                    }
                    syn::Fields::Named(named) => {
                        let of = format!("{name}::{variant_name}");
                        let (fields, keeps_unknown) = fields(named, &of)?;
                        nested.push(plain(of, Kind::Struct, fields, keeps_unknown));
                        "{}".to_owned()
                    }
                };
                let mut flags = flags_of(&variant.attrs);
                if has_schema_word(&variant.attrs, "default") {
                    flags = if flags.is_empty() {
                        " [default]".to_owned()
                    } else {
                        flags.replace(']', ", default]")
                    };
                }
                members.push((variant_name, format!("{shape}{flags}")));
            }
            if unknown > 1 || defaults > 1 {
                return Err(format!(
                    "{name}: at most one variant keeps unknown variants, and at most one is the default"
                ));
            }
            let closed = has_schema_word(&input.attrs, "closed");
            if closed && unknown == 1 {
                return Err(format!(
                    "{name}: an enum with a #[schema(unknown)] case is not #[schema(closed)]"
                ));
            }
            let mut described = plain(name, Kind::Enum, members, false);
            described.open = unknown == 1;
            described.closed = closed;
            let mut all = vec![described];
            all.extend(nested);
            Ok(all)
        }
        syn::Data::Union(_) => Err(format!("{name}: a union cannot be a Schema type")),
    }
}

/// `now`'s entry, from `old` (the lockfile's before the change). What a frozen entry cannot
/// take is added to `errors`.
pub fn update_entry(old: Option<&Entry>, now: &Described, errors: &mut Vec<String>) -> Entry {
    let key = &now.name;
    let was_frozen = old.is_some_and(Entry::is_frozen);
    if was_frozen && !now.frozen {
        errors.push(format!(
            "{key} is frozen in {FILE}, but is marked #[schema(unfrozen)]: a frozen type stays frozen"
        ));
    }
    if now.frozen && now.kind == Kind::Enum && !now.open && !now.closed {
        errors.push(format!(
            "{key} is a frozen enum, so it says whether a later build may add variants: a #[schema(unknown)] case keeps those an older build does not know, #[schema(closed)] says there will be none"
        ));
    }
    let old = old.filter(|old| {
        if old.kind == now.kind {
            return true;
        }
        if was_frozen {
            errors.push(format!(
                "{key} is frozen as a {} and cannot become a {}",
                old.kind.word(),
                now.kind.word()
            ));
        }
        false
    });
    if was_frozen && let Some(old) = old {
        if old.inner != now.inner {
            errors.push(format!(
                "{key} is frozen holding `{}` and cannot hold `{}`",
                old.inner, now.inner
            ));
        }
        if old.open && !now.open {
            errors.push(format!(
                "{key} is frozen keeping unknown variants, so it keeps its #[schema(unknown)] case"
            ));
        }
        if old.keeps_unknown && !now.keeps_unknown {
            errors.push(format!(
                "{key} is frozen keeping unknown fields, so it keeps its #[schema(unknown)] field"
            ));
        }
    }
    let mut next = old.map_or(1, Entry::next);
    let mut members = Vec::new();
    let mut added = Vec::new();
    let old_members = old.map_or(&[][..], |old| &old.members);
    for (name, shape) in &now.members {
        let kept = old_members
            .iter()
            .find(|member| !member.retired && &member.name == name);
        let id = match kept {
            Some(kept) => {
                if was_frozen && &kept.shape != shape {
                    errors.push(format!(
                        "{key}.{name} is frozen as `{name}{}` and cannot become `{name}{shape}`",
                        kept.shape
                    ));
                }
                kept.id
            }
            None => {
                if was_frozen
                    && let Some(retired) = old_members.iter().find(|member| &member.name == name)
                {
                    errors.push(format!(
                        "{key}.{name} names member {} of a frozen type, which {FILE} retired: give the new member another name, so data that meant the old one is not read as the new one",
                        retired.id
                    ));
                }
                next += 1;
                added.push(name);
                next - 1
            }
        };
        members.push(Member {
            id,
            name: name.clone(),
            shape: shape.clone(),
            retired: false,
        });
    }
    for member in old_members {
        if member.retired {
            members.push(member.clone());
        } else if !now.members.iter().any(|(name, _)| name == &member.name) {
            if was_frozen {
                errors.push(format!(
                    "{key}.{name} is frozen and no longer in the source: if it was renamed, rename it in {FILE} too; if it was removed, confirm it there by writing `{id} retired {name}{shape}` in place of `{id} {name}{shape}`",
                    name = member.name,
                    id = member.id,
                    shape = member.shape,
                ));
            }
            members.push(Member {
                retired: true,
                ..member.clone()
            });
        }
    }
    if was_frozen && now.kind == Kind::Enum && !now.open && !added.is_empty() {
        errors.push(format!(
            "{key} is a frozen enum with no #[schema(unknown)] case, so an older build could not read a new variant ({})",
            added
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    members.sort_by_key(|member| member.id);
    let frozen_from = match old.and_then(|old| old.frozen_from) {
        Some(from) => Some(from),
        None if now.frozen => Some(next),
        None => None,
    };
    Entry {
        kind: now.kind,
        inner: now.inner.clone(),
        open: now.open,
        closed: now.closed,
        keeps_unknown: now.keeps_unknown,
        frozen_from,
        members,
    }
}

/// The entries the derive numbers `input` by: the lockfile's, updated as ./scripts/verify will
/// update them, so the ids it gives are the ones the next lockfile holds.
pub fn numbered(
    input: &syn::DeriveInput,
    lock: &Lock,
) -> Result<Vec<(Described, Entry)>, Vec<String>> {
    let described = describe(input).map_err(|error| vec![error])?;
    let mut errors = Vec::new();
    let entries = described
        .into_iter()
        .map(|now| {
            let entry = update_entry(lock.types.get(&now.name), &now, &mut errors);
            (now, entry)
        })
        .collect();
    if errors.is_empty() {
        Ok(entries)
    } else {
        Err(errors)
    }
}

/// One Rust file of the crate, by its path in it.
pub struct Source<'a> {
    pub path: &'a str,
    pub text: &'a str,
}

/// The crate's lockfile after a change: `old` updated from every `Schema` type in `sources`.
/// Fails with every reason a frozen type refuses the change.
pub fn update(sources: &[Source], old: &Lock) -> Result<Lock, Vec<String>> {
    let mut errors = Vec::new();
    let mut found: Vec<(Described, &str)> = Vec::new();
    for source in sources {
        let file = match syn::parse_file(source.text) {
            Ok(file) => file,
            Err(error) => {
                errors.push(format!("{}: {error}", source.path));
                continue;
            }
        };
        let mut inputs = Vec::new();
        collect(&file.items, &mut inputs);
        for input in inputs {
            match describe(&input) {
                Ok(described) => {
                    found.extend(described.into_iter().map(|d| (d, source.path)));
                }
                Err(error) => errors.push(format!("{}: {error}", source.path)),
            }
        }
    }
    let mut lock = Lock::default();
    let mut paths: BTreeMap<&str, &str> = BTreeMap::new();
    for (described, path) in &found {
        if let Some(first) = paths.insert(&described.name, path) {
            errors.push(format!(
                "{} is a Schema type in both {first} and {path}: a crate's Schema types have one name each",
                described.name
            ));
            continue;
        }
        let entry = update_entry(old.types.get(&described.name), described, &mut errors);
        lock.types.insert(described.name.clone(), entry);
    }
    for (name, entry) in &old.types {
        let parent = name.split_once("::").map(|(parent, _)| parent);
        let gone = !lock.types.contains_key(name)
            && parent.is_none_or(|parent| !lock.types.contains_key(parent));
        if gone && entry.is_frozen() {
            errors.push(format!(
                "{name} is frozen in {FILE} and no longer in the source: if it was renamed, rename it in {FILE} too; a frozen type is never removed"
            ));
        }
    }
    if errors.is_empty() {
        Ok(lock)
    } else {
        errors.sort();
        errors.dedup();
        Err(errors)
    }
}

fn collect(items: &[syn::Item], out: &mut Vec<syn::DeriveInput>) {
    for item in items {
        match item {
            syn::Item::Struct(item) if derives_schema(&item.attrs) => {
                out.push(item.clone().into());
            }
            syn::Item::Enum(item) if derives_schema(&item.attrs) => {
                out.push(item.clone().into());
            }
            syn::Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    collect(items, out);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
