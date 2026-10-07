use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::compiler::Symbol;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperatorKind {
    Slot,
    Lhs,
}

#[derive(Default)]
struct Interned {
    by_operator: HashMap<(OperatorKind, String), Symbol>,
    by_symbol: HashMap<Symbol, (OperatorKind, String)>,
}

fn interned() -> &'static Mutex<Interned> {
    static INTERNED: OnceLock<Mutex<Interned>> = OnceLock::new();
    INTERNED.get_or_init(Default::default)
}

pub fn operator_symbol(kind: OperatorKind, op: &str) -> Symbol {
    let mut interned = interned().lock().expect("operator interner poisoned");
    if let Some(symbol) = interned.by_operator.get(&(kind, op.to_string())) {
        return *symbol;
    }
    let symbol = Symbol::new();
    interned.by_operator.insert((kind, op.to_string()), symbol);
    interned.by_symbol.insert(symbol, (kind, op.to_string()));
    symbol
}

pub fn symbol_operator(symbol: Symbol) -> Option<(OperatorKind, String)> {
    interned()
        .lock()
        .expect("operator interner poisoned")
        .by_symbol
        .get(&symbol)
        .cloned()
}
