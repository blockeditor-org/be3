use super::*;

mod a_character_key_is_its_letter_or_digit_or_else_its_place;
mod a_layout_from_the_config_moves_the_letters;
mod a_shifted_letter_types_its_capital;
mod an_unknown_layout_does_not_compile;
mod control_alt_and_a_function_key_switch_terminals;
mod super_is_its_own_modifier;

const SHIFT: u32 = 42;
const CONTROL: u32 = 29;
const ALT: u32 = 56;
const KEY_A: u32 = 30;
const KEY_1: u32 = 2;
const KEY_Q: u32 = 16;
const KEY_M: u32 = 50;
const KEY_Y: u32 = 21;
const KEY_F2: u32 = 60;
const KEY_F: u32 = 33;
const LEFT_SUPER: u32 = 125;
