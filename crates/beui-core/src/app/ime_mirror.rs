use std::ops::Range;

use crate::input::{Event, ImeEvent, ImeText};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImeMirror {
    start: usize,
    value: String,
    selection: Range<usize>,
    composition: Option<Range<usize>>,
    composing: bool,
}

pub struct ImeInput<'a> {
    pub value: &'a str,
    pub selection: (u32, u32),
    pub composing: bool,
    pub replacement: bool,
}

impl ImeMirror {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn selection(&self) -> (u32, u32) {
        (
            utf16(&self.value, self.selection.start - self.start),
            utf16(&self.value, self.selection.end - self.start),
        )
    }

    pub fn sync(&mut self, text: Option<&ImeText>) -> bool {
        if self.composing {
            return false;
        }
        let next = match text {
            Some(text) => Self {
                start: text.start,
                value: text.text.clone(),
                selection: text.selection.clone(),
                composition: text.composing.clone(),
                composing: false,
            },
            None => Self::default(),
        };
        let changed = next.start != self.start
            || next.value != self.value
            || next.selection != self.selection;
        *self = next;
        changed
    }

    pub fn start_composition(&mut self) {
        self.composing = true;
    }

    pub fn end_composition(&mut self, events: &mut Vec<Event>) {
        self.composing = false;
        if self.composition.take().is_some() {
            events.push(Event::Ime(ImeEvent::FinishComposing));
        }
    }

    pub fn select(&mut self, selection: (u32, u32), events: &mut Vec<Event>) {
        if self.composing {
            return;
        }
        let anchor = self.start + byte(&self.value, selection.0);
        let focus = self.start + byte(&self.value, selection.1);
        let selection = anchor.min(focus)..anchor.max(focus);
        if selection != self.selection {
            self.selection = selection;
            events.push(Event::Ime(ImeEvent::SetSelection { anchor, focus }));
        }
    }

    pub fn input(&mut self, input: ImeInput<'_>, events: &mut Vec<Event>) {
        let held = [Some(self.selection.clone()), self.composition.clone()];
        let low = held
            .iter()
            .flatten()
            .map(|range| range.start)
            .min()
            .unwrap_or(self.start);
        let high = held
            .iter()
            .flatten()
            .map(|range| range.end)
            .max()
            .unwrap_or(self.start);
        let (local, inserted) = diff(
            &self.value,
            input.value,
            low - self.start,
            self.value.len() - (high - self.start),
        );
        let range = self.start + local.start..self.start + local.end;
        let inserted = inserted.to_owned();
        let changed = !range.is_empty() || !inserted.is_empty();
        let end = range.start + inserted.len();
        if changed {
            self.edit(range, inserted, &input, events);
        }
        self.value = input.value.to_owned();
        let anchor = self.start + byte(&self.value, input.selection.0);
        let focus = self.start + byte(&self.value, input.selection.1);
        let expected = match changed {
            true => end..end,
            false => self.selection.clone(),
        };
        self.selection = anchor.min(focus)..anchor.max(focus);
        if self.selection != expected {
            events.push(Event::Ime(ImeEvent::SetSelection { anchor, focus }));
        }
    }

    fn edit(
        &mut self,
        range: Range<usize>,
        inserted: String,
        input: &ImeInput<'_>,
        events: &mut Vec<Event>,
    ) {
        let end = range.start + inserted.len();
        if input.composing {
            match &self.composition {
                Some(composition) if *composition == range => {}
                None if range == self.selection => {}
                held if range.is_empty() => {
                    if held.is_some() {
                        events.push(Event::Ime(ImeEvent::FinishComposing));
                    }
                    events.push(Event::Ime(ImeEvent::SetSelection {
                        anchor: range.start,
                        focus: range.start,
                    }));
                }
                _ => events.push(Event::Ime(ImeEvent::SetComposingRegion(range.clone()))),
            }
            self.composition = (!inserted.is_empty()).then_some(range.start..end);
            events.push(Event::Ime(ImeEvent::SetComposingText(inserted)));
        } else if input.replacement {
            self.composition = None;
            events.push(Event::Ime(ImeEvent::ReplaceText {
                range,
                text: inserted,
            }));
        } else if self.composition.as_ref() == Some(&range) || range == self.selection {
            self.composition = None;
            events.push(Event::Ime(ImeEvent::CommitText(inserted)));
        } else if inserted.is_empty()
            && self.selection.is_empty()
            && range.start <= self.selection.start
            && self.selection.start <= range.end
        {
            events.push(Event::Ime(ImeEvent::DeleteSurrounding {
                before: self.selection.start - range.start,
                after: range.end - self.selection.start,
            }));
        } else {
            self.composition = None;
            events.push(Event::Ime(ImeEvent::ReplaceText {
                range,
                text: inserted,
            }));
        }
    }
}

fn diff<'a>(old: &str, new: &'a str, prefix: usize, suffix: usize) -> (Range<usize>, &'a str) {
    let mut start = 0;
    for ((index, before), after) in old.char_indices().zip(new.chars()) {
        if before != after || index >= prefix {
            break;
        }
        start = index + before.len_utf8();
    }
    let start = start.min(prefix);
    let mut tail = 0;
    for (before, after) in old[start..].chars().rev().zip(new[start..].chars().rev()) {
        if before != after || tail + before.len_utf8() > suffix {
            break;
        }
        tail += before.len_utf8();
    }
    (start..old.len() - tail, &new[start..new.len() - tail])
}

fn byte(text: &str, utf16_index: u32) -> usize {
    let mut units = 0;
    for (index, letter) in text.char_indices() {
        if units >= utf16_index as usize {
            return index;
        }
        units += letter.len_utf16();
    }
    text.len()
}

fn utf16(text: &str, byte_index: usize) -> u32 {
    let index = byte_index.min(text.len());
    let units: usize = text
        .get(..index)
        .map_or(0, |head| head.chars().map(char::len_utf16).sum());
    u32::try_from(units).unwrap_or(u32::MAX)
}
