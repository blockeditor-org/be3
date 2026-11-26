//! `schema.lock`: the ids of a crate's `Schema` types (be-wasm-schema).
//!
//! The source says what a type is and whether it is frozen (its file calls
//! `be_wasm_schema::frozen!()`); the lockfile says which id each field and variant has, so the
//! source may reorder them freely. ./scripts/verify rewrites the lockfile from the source and
//! the lockfile before it (`update`), and the derive numbers a type from the lockfile checked in
//! (`numbered`) the same way, so a build never waits for the lockfile to catch up.
//!
//! Every entry is updated incrementally: a member keeps its id while its name stays, a new one
//! takes the next unused id, and a removed one stays as `retired` so its id is never given
//! again. An unfrozen entry accepts every change. A frozen one refuses what an older build could
//! not read: a retyped member, a frozen type made unfrozen or removed, a variant added to an
//! enum with no unknown case, and a retired name given again. A frozen member is removed or
//! renamed only with a hand edit here, so neither happens by accident: removing one means
//! marking it `retired` (an API that forgot a field that meant something would answer wrongly
//! rather than fail), and renaming one means renaming it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;

/// The lockfile's name, beside the crate's `Cargo.toml`.
pub const FILE: &str = "schema.lock";

const HEADER: &str = "\
# Written by ./scripts/verify from this crate's Schema types (be-wasm-schema): the id of each
# field and variant. Rename a member of a frozen type here when you rename it in the source.
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
    /// for a struct variant (whose fields are the entry `Enum::Variant`). Empty when retired.
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
                let (id, rest) = member.split_once(' ').ok_or_else(|| fail("no id"))?;
                let id: u32 = id.parse().map_err(|_| fail("not an id"))?;
                let end = rest
                    .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '#'))
                    .unwrap_or(rest.len());
                let (name, shape) = rest.split_at(end);
                let retired = shape == " retired";
                if id == 0 || entry.members.iter().any(|member| member.id == id) {
                    return Err(fail("an id given twice, or zero"));
                }
                entry.members.push(Member {
                    id,
                    name: name.to_owned(),
                    shape: if retired {
                        String::new()
                    } else {
                        shape.to_owned()
                    },
                    retired,
                });
                continue;
            }
            if let Some((name, entry)) = current.take() {
                lock.insert(name, entry);
            }
            let (head, flags) = match line.split_once(" (") {
                Some((head, flags)) => (
                    head,
                    flags
                        .strip_suffix(')')
                        .ok_or_else(|| fail("unclosed flags"))?,
                ),
                None => (line, ""),
            };
            let (word, rest) = head.split_once(' ').ok_or_else(|| fail("not a type"))?;
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
            let mut entry = Entry {
                kind,
                inner: inner.to_owned(),
                open: false,
                frozen_from: None,
                members: Vec::new(),
            };
            for flag in flags.split(", ").filter(|flag| !flag.is_empty()) {
                if flag == "open" {
                    entry.open = true;
                } else if let Some(from) = flag.strip_prefix("frozen from ") {
                    entry.frozen_from = Some(from.parse().map_err(|_| fail("not an id"))?);
                } else {
                    return Err(fail("an unknown flag"));
                }
            }
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
            let mut flags = Vec::new();
            if entry.open {
                flags.push("open".to_owned());
            }
            if let Some(from) = entry.frozen_from {
                flags.push(format!("frozen from {from}"));
            }
            if !flags.is_empty() {
                let _ = write!(text, " ({})", flags.join(", "));
            }
            text.push('\n');
            for member in &entry.members {
                let shape = if member.retired {
                    " retired"
                } else {
                    &member.shape
                };
                let _ = writeln!(text, "  {} {}{shape}", member.id, member.name);
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
    pub open: bool,
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

/// A type as the lockfile writes it: tokens with spaces only between words.
pub fn shape_of(ty: &syn::Type) -> String {
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
    walk(ty.to_token_stream(), &mut out, &mut false);
    out
}

/// What `input` is, refused where the format cannot carry it: a type with generics other
/// than one lifetime, a tuple struct or variant of more than one field, more than one unknown
/// case or field, or a default case that carries data. A struct variant adds an entry of its
/// own, `Enum::Variant`, after its enum.
pub fn describe(input: &syn::DeriveInput) -> Result<Vec<Described>, String> {
    let name = input.ident.to_string();
    let generics = &input.generics.params;
    if generics
        .iter()
        .any(|param| !matches!(param, syn::GenericParam::Lifetime(_)))
        || generics.len() > 1
    {
        return Err(format!(
            "{name}: a Schema type takes at most one lifetime and no type parameters"
        ));
    }
    let fields = |fields: &syn::FieldsNamed, of: &str| -> Result<Vec<(String, String)>, String> {
        let mut members = Vec::new();
        let mut unknown = 0;
        for field in &fields.named {
            if has_schema_word(&field.attrs, "unknown") {
                unknown += 1;
                continue;
            }
            let ident = field.ident.as_ref().map(ToString::to_string);
            members.push((
                ident.unwrap_or_default(),
                format!(": {}", shape_of(&field.ty)),
            ));
        }
        if unknown > 1 {
            return Err(format!("{of}: at most one field keeps unknown fields"));
        }
        Ok(members)
    };
    match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(named) => Ok(vec![Described {
                name: name.clone(),
                kind: Kind::Struct,
                inner: String::new(),
                open: false,
                members: fields(named, &name)?,
            }]),
            syn::Fields::Unit => Ok(vec![Described {
                name,
                kind: Kind::Struct,
                inner: String::new(),
                open: false,
                members: Vec::new(),
            }]),
            syn::Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1 => Ok(vec![Described {
                name,
                kind: Kind::Newtype,
                inner: shape_of(&unnamed.unnamed[0].ty),
                open: false,
                members: Vec::new(),
            }]),
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
                        format!("({})", shape_of(&unnamed.unnamed[0].ty))
                    }
                    syn::Fields::Unnamed(_) => {
                        return Err(format!(
                            "{name}::{variant_name}: a variant of more than one unnamed field has no names to number; make it a struct variant"
                        ));
                    }
                    syn::Fields::Named(named) => {
                        let of = format!("{name}::{variant_name}");
                        nested.push(Described {
                            name: of.clone(),
                            kind: Kind::Struct,
                            inner: String::new(),
                            open: false,
                            members: fields(named, &of)?,
                        });
                        "{}".to_owned()
                    }
                };
                members.push((variant_name, shape));
            }
            if unknown > 1 || defaults > 1 {
                return Err(format!(
                    "{name}: at most one variant keeps unknown variants, and at most one is the default"
                ));
            }
            let mut described = vec![Described {
                name,
                kind: Kind::Enum,
                inner: String::new(),
                open: unknown == 1,
                members,
            }];
            described.extend(nested);
            Ok(described)
        }
        syn::Data::Union(_) => Err(format!("{name}: a union cannot be a Schema type")),
    }
}

/// `now`'s entry, from `old` (the lockfile's before the change), frozen when its file is.
/// What a frozen entry cannot take is added to `errors`.
pub fn update_entry(
    old: Option<&Entry>,
    now: &Described,
    frozen: bool,
    errors: &mut Vec<String>,
) -> Entry {
    let key = &now.name;
    let was_frozen = old.is_some_and(Entry::is_frozen);
    if was_frozen && !frozen {
        errors.push(format!(
            "{key} is frozen in {FILE}, but its file no longer calls be_wasm_schema::frozen!(): a frozen type stays frozen, in a frozen file"
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
    }
    let mut next = old.map_or(1, Entry::next);
    let mut members = Vec::new();
    let mut added = Vec::new();
    for (name, shape) in &now.members {
        let old_members = old.map_or(&[][..], |old| &old.members);
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
    for member in old.map_or(&[][..], |old| &old.members) {
        if member.retired {
            members.push(member.clone());
        } else if !now.members.iter().any(|(name, _)| name == &member.name) {
            if was_frozen {
                errors.push(format!(
                    "{key}.{} is frozen and no longer in the source: if it was renamed, rename it in {FILE} too; if it was removed, confirm it there by replacing `{} {}{}` with `{} {} retired`",
                    member.name, member.id, member.name, member.shape, member.id, member.name
                ));
            }
            members.push(Member {
                id: member.id,
                name: member.name.clone(),
                shape: String::new(),
                retired: true,
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
        None if frozen => Some(next),
        None => None,
    };
    Entry {
        kind: now.kind,
        inner: now.inner.clone(),
        open: now.open,
        frozen_from,
        members,
    }
}

/// The entries the derive numbers `input` by: the lockfile's, updated as ./scripts/verify will
/// update them, so the ids it gives are the ones the next lockfile holds. A type counts as
/// frozen here once the lockfile says so; only `update` sees the file's `frozen!()`.
pub fn numbered(
    input: &syn::DeriveInput,
    lock: &Lock,
) -> Result<Vec<(Described, Entry)>, Vec<String>> {
    let described = describe(input).map_err(|error| vec![error])?;
    let mut errors = Vec::new();
    let entries = described
        .into_iter()
        .map(|now| {
            let old = lock.types.get(&now.name);
            let entry = update_entry(old, &now, old.is_some_and(Entry::is_frozen), &mut errors);
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
/// Fails with every reason a frozen type refuses the change, and when a frozen type holds a
/// type of the crate that is not frozen.
pub fn update(sources: &[Source], old: &Lock) -> Result<Lock, Vec<String>> {
    let mut errors = Vec::new();
    let mut found: Vec<(Described, bool, &str)> = Vec::new();
    for source in sources {
        let file = match syn::parse_file(source.text) {
            Ok(file) => file,
            Err(error) => {
                errors.push(format!("{}: {error}", source.path));
                continue;
            }
        };
        let frozen = file.items.iter().any(|item| {
            matches!(item, syn::Item::Macro(call)
                if call.mac.path.segments.last().is_some_and(|segment| segment.ident == "frozen"))
        });
        let mut inputs = Vec::new();
        collect(&file.items, &mut inputs);
        for input in inputs {
            match describe(&input) {
                Ok(described) => {
                    found.extend(described.into_iter().map(|d| (d, frozen, source.path)));
                }
                Err(error) => errors.push(format!("{}: {error}", source.path)),
            }
        }
    }
    let mut lock = Lock::default();
    let mut paths: BTreeMap<&str, &str> = BTreeMap::new();
    for (described, frozen, path) in &found {
        if let Some(first) = paths.insert(&described.name, path) {
            errors.push(format!(
                "{} is a Schema type in both {first} and {path}: a crate's Schema types have one name each",
                described.name
            ));
            continue;
        }
        let entry = update_entry(
            old.types.get(&described.name),
            described,
            *frozen,
            &mut errors,
        );
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
    let names: BTreeSet<&str> = lock
        .types
        .keys()
        .filter(|name| !name.contains("::"))
        .map(String::as_str)
        .collect();
    for (name, entry) in &lock.types {
        if !entry.is_frozen() {
            continue;
        }
        let shapes = entry
            .members
            .iter()
            .map(|member| member.shape.as_str())
            .chain([entry.inner.as_str()]);
        for shape in shapes {
            for word in shape.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
                if names.contains(word) && !lock.types[word].is_frozen() {
                    errors.push(format!(
                        "{name} is frozen and holds {word}, which is not: freeze {word} too (call be_wasm_schema::frozen!() in its file)"
                    ));
                }
            }
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
