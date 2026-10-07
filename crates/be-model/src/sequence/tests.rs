use std::collections::{BTreeMap, HashMap};

use super::{CHUNK, LOADED, Pos, SeqOp, Sequence, Span, Splice};

mod a_delete_leaves_text_inserted_inside_it_at_the_same_time;
mod a_move_into_itself_or_with_its_ends_out_of_order_changes_nothing;
mod a_move_takes_text_inserted_inside_it_along;
mod an_insert_after_a_deleted_element_lands_where_it_was;
mod an_insert_with_the_wrong_start_or_an_unknown_anchor_is_refused;
mod bytes_that_are_not_utf8_are_kept_as_they_are;
mod inserts_after_the_same_element_put_the_later_one_first;
mod loaded_items_read_back_as_one_fragment;
mod random_edits_match_a_reference_and_keep_the_index_consistent;
mod session_state_round_trips_positions_and_tombstones;
mod splices_report_visible_coordinates;
mod the_saved_form_is_only_the_visible_items;
mod two_people_checking_the_same_box_check_it_once;
mod typing_extends_one_fragment;
mod undoing_a_delete_brings_back_the_same_positions;
mod undoing_a_move_puts_the_range_back;
mod undoing_a_replace_swaps_back_and_redo_swaps_again;

const ALICE: u64 = 1;
const BOB: u64 = 2;

fn loaded(text: &str) -> Sequence<u8> {
    Sequence::from_items(text.as_bytes().to_vec())
}

fn text(sequence: &Sequence<u8>) -> String {
    String::from_utf8_lossy(&sequence.items()).into_owned()
}

fn applied(sequence: &mut Sequence<u8>, op: &SeqOp<u8>) -> Vec<Splice> {
    sequence.apply(op).expect("the operation applies")
}

fn typed(sequence: &mut Sequence<u8>, client: u64, index: usize, typed: &str) -> SeqOp<u8> {
    let op = sequence
        .insert(client, index, typed.as_bytes().to_vec())
        .expect("there is something to insert");
    applied(sequence, &op);
    op
}

fn check(sequence: &Sequence<u8>) {
    let mut seen = BTreeMap::new();
    for (at, chunk) in sequence.chunks.iter().enumerate() {
        assert_eq!(sequence.order.get(&chunk.key), Some(&at));
        assert_eq!(
            chunk.visible,
            chunk
                .fragments
                .iter()
                .map(|fragment| fragment.visible_len())
                .sum::<usize>()
        );
        assert!(chunk.fragments.len() <= 2 * CHUNK);
        for fragment in &chunk.fragments {
            assert!(fragment.len > 0);
            assert_eq!(sequence.index.get(&fragment.first()), Some(&chunk.key));
            assert!(seen.insert(fragment.first(), fragment.len).is_none());
        }
    }
    assert_eq!(sequence.index.len(), seen.len());
}

fn order(sequence: &Sequence<u8>) -> Vec<(Pos, u8, bool)> {
    sequence
        .fragments()
        .flat_map(|fragment| {
            sequence
                .slice(*fragment)
                .iter()
                .enumerate()
                .map(|(at, item)| {
                    (
                        Pos {
                            client: fragment.client,
                            offset: fragment.start + at as u32,
                        },
                        *item,
                        fragment.visible,
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

struct Reference {
    order: Vec<(Pos, u8, bool)>,
    next: BTreeMap<u64, u32>,
    indices: HashMap<Pos, usize>,
}

impl Reference {
    fn new(text: &str) -> Self {
        Self {
            order: text
                .bytes()
                .enumerate()
                .map(|(at, item)| {
                    (
                        Pos {
                            client: LOADED,
                            offset: at as u32,
                        },
                        item,
                        true,
                    )
                })
                .collect(),
            next: BTreeMap::from([(LOADED, text.len() as u32)]),
            indices: (0..text.len())
                .map(|at| {
                    (
                        Pos {
                            client: LOADED,
                            offset: at as u32,
                        },
                        at,
                    )
                })
                .collect(),
        }
    }

    fn at(&self, pos: Pos) -> Option<usize> {
        self.indices.get(&pos).copied()
    }

    fn reindex(&mut self) {
        self.indices = self
            .order
            .iter()
            .enumerate()
            .map(|(at, (pos, _, _))| (*pos, at))
            .collect();
    }

    fn positions(&self, span: Span) -> Option<Vec<usize>> {
        if span.len == 0
            || span.start + span.len > self.next.get(&span.client).copied().unwrap_or(0)
        {
            return None;
        }
        (span.start..span.start + span.len)
            .map(|offset| {
                self.at(Pos {
                    client: span.client,
                    offset,
                })
            })
            .collect()
    }

    fn all(&self, spans: &[Span], visible: bool) -> bool {
        spans.iter().all(|span| {
            self.positions(*span)
                .is_some_and(|at| at.iter().all(|index| self.order[*index].2 == visible))
        })
    }

    fn set(&mut self, spans: &[Span], visible: bool) -> bool {
        let mut changed = false;
        for span in spans {
            for index in self.positions(*span).unwrap_or_default() {
                changed |= self.order[index].2 != visible;
                self.order[index].2 = visible;
            }
        }
        changed
    }

    fn add(&mut self, after: Option<Pos>, client: u64, start: u32, items: &[u8]) -> bool {
        let next = self.next.get(&client).copied().unwrap_or(0);
        if items.is_empty() || start != next {
            return false;
        }
        let index = match after {
            None => 0,
            Some(anchor) => match self.at(anchor) {
                Some(index) => index + 1,
                None => return false,
            },
        };
        let added = items.iter().enumerate().map(|(at, item)| {
            (
                Pos {
                    client,
                    offset: start + at as u32,
                },
                *item,
                true,
            )
        });
        self.order.splice(index..index, added);
        self.next.insert(client, next + items.len() as u32);
        self.reindex();
        true
    }

    fn apply(&mut self, op: &SeqOp<u8>) -> bool {
        match op {
            SeqOp::Insert {
                after,
                client,
                start,
                items,
            } => self.add(*after, *client, *start, items),
            SeqOp::Delete { spans } => self.set(spans, false),
            SeqOp::Undelete { spans } => self.set(spans, true),
            SeqOp::Replace {
                spans,
                client,
                start,
                items,
            } => {
                let next = self.next.get(client).copied().unwrap_or(0);
                let Some(last) = spans.last() else {
                    return false;
                };
                if !self.all(spans, true) || *start != next {
                    return false;
                }
                self.set(spans, false);
                if !items.is_empty() {
                    let after = Pos {
                        client: last.client,
                        offset: last.start + last.len - 1,
                    };
                    self.add(Some(after), *client, *start, items);
                }
                true
            }
            SeqOp::Swap { hide, show } => {
                if (hide.is_empty() && show.is_empty())
                    || !self.all(hide, true)
                    || !self.all(show, false)
                {
                    return false;
                }
                self.set(hide, false);
                self.set(show, true);
                true
            }
            SeqOp::Move { first, last, after } => {
                let (Some(start), Some(end)) = (self.at(*first), self.at(*last)) else {
                    return false;
                };
                if start > end {
                    return false;
                }
                let target = match after {
                    None => None,
                    Some(anchor) => match self.at(*anchor) {
                        Some(index) => Some(index),
                        None => return false,
                    },
                };
                if target.is_some_and(|index| start <= index && index <= end) {
                    return false;
                }
                let previous = start.checked_sub(1).map(|index| self.order[index].0);
                if previous == *after {
                    return false;
                }
                let moved: Vec<_> = self.order.drain(start..=end).collect();
                self.reindex();
                let index = match after {
                    None => 0,
                    Some(anchor) => self.at(*anchor).map_or(0, |index| index + 1),
                };
                self.order.splice(index..index, moved);
                self.reindex();
                true
            }
        }
    }
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: usize) -> usize {
        match bound {
            0 => 0,
            _ => (self.next() % bound as u64) as usize,
        }
    }
}
