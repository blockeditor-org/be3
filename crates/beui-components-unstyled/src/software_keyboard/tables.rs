use beui_core::input::Key;

use super::layout::{Layer, Value};

const FUNCTION: [Key; 10] = [
    Key::F10,
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
    Key::F9,
];

const FN_CHARS: &[(char, char)] = &[
    ('<', '«'),
    ('>', '»'),
    ('{', '‹'),
    ('}', '›'),
    ('[', '‘'),
    (']', '’'),
    ('(', '“'),
    (')', '”'),
    ('\'', '‚'),
    ('"', '„'),
    ('-', '–'),
    ('_', '—'),
    ('^', '¬'),
    ('%', '‰'),
    ('=', '≈'),
    ('u', 'µ'),
    ('a', 'æ'),
    ('o', 'œ'),
    ('*', '°'),
    ('.', '…'),
    (',', '·'),
    ('!', '¡'),
    ('?', '¿'),
    ('|', '¦'),
    ('§', '¶'),
    ('†', '‡'),
    ('×', '∙'),
    (' ', '\u{a0}'),
    ('↑', '⇑'),
    ('←', '⇐'),
    ('→', '⇒'),
    ('↓', '⇓'),
    ('↔', '⇔'),
    ('↕', '⇕'),
    ('e', '€'),
    ('l', '£'),
    ('r', '₹'),
    ('y', '¥'),
    ('c', '¢'),
    ('p', '₽'),
    ('b', '₱'),
    ('h', '₴'),
    ('z', '₿'),
    ('π', 'ϖ'),
    ('θ', 'ϑ'),
    ('ε', 'ϵ'),
    ('β', 'ϐ'),
    ('ρ', 'ϱ'),
    ('σ', 'ς'),
    ('γ', 'ɣ'),
    ('φ', 'ϕ'),
    ('υ', 'ϒ'),
    ('κ', 'ϰ'),
    ('∪', '⋃'),
    ('∩', '⋂'),
    ('∃', '∄'),
    ('∫', '∮'),
    ('Σ', '∑'),
    ('∨', '⋁'),
    ('∧', '⋀'),
    ('⊷', '⊶'),
    ('⊂', '⊆'),
    ('⊃', '⊇'),
    ('±', '∓'),
];

const SUPERSCRIPT: &[(char, char)] = &[
    ('0', '⁰'),
    ('1', '¹'),
    ('2', '²'),
    ('3', '³'),
    ('4', '⁴'),
    ('5', '⁵'),
    ('6', '⁶'),
    ('7', '⁷'),
    ('8', '⁸'),
    ('9', '⁹'),
    ('+', '⁺'),
    ('-', '⁻'),
    ('=', '⁼'),
    ('(', '⁽'),
    (')', '⁾'),
    ('a', 'ᵃ'),
    ('b', 'ᵇ'),
    ('c', 'ᶜ'),
    ('d', 'ᵈ'),
    ('e', 'ᵉ'),
    ('f', 'ᶠ'),
    ('g', 'ᵍ'),
    ('h', 'ʰ'),
    ('i', 'ⁱ'),
    ('j', 'ʲ'),
    ('k', 'ᵏ'),
    ('l', 'ˡ'),
    ('m', 'ᵐ'),
    ('n', 'ⁿ'),
    ('o', 'ᵒ'),
    ('p', 'ᵖ'),
    ('q', 'ꟴ'),
    ('r', 'ʳ'),
    ('s', 'ˢ'),
    ('t', 'ᵗ'),
    ('u', 'ᵘ'),
    ('v', 'ᵛ'),
    ('w', 'ʷ'),
    ('x', 'ˣ'),
    ('y', 'ʸ'),
    ('z', 'ᶻ'),
    ('β', 'ᵝ'),
    ('δ', 'ᵟ'),
    ('φ', 'ᵠ'),
    ('χ', 'ᵡ'),
    ('ι', 'ᶥ'),
    ('θ', 'ᶿ'),
];

const SUBSCRIPT: &[(char, char)] = &[
    ('0', '₀'),
    ('1', '₁'),
    ('2', '₂'),
    ('3', '₃'),
    ('4', '₄'),
    ('5', '₅'),
    ('6', '₆'),
    ('7', '₇'),
    ('8', '₈'),
    ('9', '₉'),
    ('+', '₊'),
    ('-', '₋'),
    ('=', '₌'),
    ('(', '₍'),
    (')', '₎'),
    ('a', 'ₐ'),
    ('e', 'ₑ'),
    ('h', 'ₕ'),
    ('i', 'ᵢ'),
    ('j', 'ⱼ'),
    ('k', 'ₖ'),
    ('l', 'ₗ'),
    ('m', 'ₘ'),
    ('n', 'ₙ'),
    ('o', 'ₒ'),
    ('p', 'ₚ'),
    ('r', 'ᵣ'),
    ('s', 'ₛ'),
    ('t', 'ₜ'),
    ('u', 'ᵤ'),
    ('v', 'ᵥ'),
    ('x', 'ₓ'),
    ('β', 'ᵦ'),
    ('γ', 'ᵧ'),
    ('ρ', 'ᵨ'),
    ('φ', 'ᵩ'),
    ('χ', 'ᵪ'),
];

fn look_up(table: &[(char, char)], character: char) -> Option<char> {
    table
        .iter()
        .find(|(from, _)| *from == character)
        .map(|(_, to)| *to)
}

pub fn function(value: Value) -> Value {
    match value {
        Value::Char(digit @ '0'..='9') => Value::Key(FUNCTION[digit as usize - '0' as usize]),
        Value::Char(character) => look_up(FN_CHARS, character).map_or(value, Value::Char),
        Value::Key(Key::ArrowUp) => Value::Key(Key::PageUp),
        Value::Key(Key::ArrowDown) => Value::Key(Key::PageDown),
        Value::Key(Key::ArrowLeft) => Value::Key(Key::Home),
        Value::Key(Key::ArrowRight) => Value::Key(Key::End),
        Value::Key(Key::Escape) => Value::Key(Key::Insert),
        Value::Key(Key::Tab) => Value::Char('\t'),
        Value::Layer(Layer::Numpad) => Value::Layer(Layer::Greek),
        value => value,
    }
}

pub fn superscript(value: Value) -> Value {
    match value {
        Value::Char(character) => look_up(SUPERSCRIPT, character).map_or(value, Value::Char),
        value => value,
    }
}

pub fn subscript(value: Value) -> Value {
    match value {
        Value::Char(character) => look_up(SUBSCRIPT, character).map_or(value, Value::Char),
        value => value,
    }
}

pub fn shifted(value: Value) -> Value {
    match value {
        Value::Char(character) => {
            let mut upper = character.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(upper), None) => Value::Char(upper),
                _ => value,
            }
        }
        value => value,
    }
}
