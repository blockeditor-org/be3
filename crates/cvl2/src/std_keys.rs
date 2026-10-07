use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::compiler::Symbol;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorKind {
    Slot,
    Lhs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiteralKind {
    String,
    Number,
    List,
    Map,
}

impl LiteralKind {
    pub fn name(self) -> &'static str {
        match self {
            LiteralKind::String => "string",
            LiteralKind::Number => "number",
            LiteralKind::List => "list",
            LiteralKind::Map => "map",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StdKey {
    Operator(OperatorKind, String),
    Literal(LiteralKind),
    Call,
}

impl StdKey {
    pub fn describe(&self) -> String {
        match self {
            StdKey::Operator(OperatorKind::Slot, op) => format!("std.operator.slot(\"{op}\")"),
            StdKey::Operator(OperatorKind::Lhs, op) => format!("std.operator.lhs(\"{op}\")"),
            StdKey::Literal(kind) => format!("std.literal.{}", kind.name()),
            StdKey::Call => "std.operator.call".to_string(),
        }
    }
}

#[derive(Default)]
struct Interned {
    by_key: HashMap<StdKey, Symbol>,
    by_symbol: HashMap<Symbol, StdKey>,
}

fn interned() -> &'static Mutex<Interned> {
    static INTERNED: OnceLock<Mutex<Interned>> = OnceLock::new();
    INTERNED.get_or_init(Default::default)
}

pub fn std_key(key: StdKey) -> Symbol {
    let mut interned = interned().lock().expect("std key interner poisoned");
    if let Some(symbol) = interned.by_key.get(&key) {
        return *symbol;
    }
    let symbol = Symbol::new();
    interned.by_key.insert(key.clone(), symbol);
    interned.by_symbol.insert(symbol, key);
    symbol
}

pub fn symbol_std_key(symbol: Symbol) -> Option<StdKey> {
    interned()
        .lock()
        .expect("std key interner poisoned")
        .by_symbol
        .get(&symbol)
        .cloned()
}

pub fn operator_symbol(kind: OperatorKind, op: &str) -> Symbol {
    std_key(StdKey::Operator(kind, op.to_string()))
}

pub fn symbol_operator(symbol: Symbol) -> Option<(OperatorKind, String)> {
    match symbol_std_key(symbol)? {
        StdKey::Operator(kind, op) => Some((kind, op)),
        _ => None,
    }
}
