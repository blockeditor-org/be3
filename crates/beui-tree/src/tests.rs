use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::reactive::{
    ChildList, ChildValue, Children, ForEach, Get, IntoSlotHost, Prop, Scope, Set, Show, Signal,
    SlotChild, SlotHost, SlotId, component, create_effect, create_memo, create_signal, flush,
    on_cleanup, store, view,
};

mod a_binding_prop_reads_and_writes_the_signal_it_was_given_or_its_own;
mod a_callback_given_a_signal_sets_it;
mod a_child_list_refilled_keeps_the_items_it_held_wherever_they_move;
mod a_closure_is_a_prop_read_as_a_getter;
mod a_derived_binding_reads_its_prop_and_writes_through_its_callback;
mod a_fallback_given_after_the_children_or_outside_view_is_inferred;
mod a_for_each_builds_a_row_for_each_repeat_of_an_item;
mod a_for_each_handle_follows_each_key_across_a_reorder_an_insert_and_a_removal;
mod a_for_each_handle_is_empty_once_its_list_is_gone;
mod a_for_each_handle_knows_a_repeated_key_by_its_first_place_and_holds_every_repeat;
mod a_for_each_keeps_its_fallback_when_a_row_changes_as_the_list_empties;
mod a_for_each_shows_its_fallback_while_it_has_no_keys;
mod a_kept_alive_show_keeps_both_branches_and_pauses_the_hidden_one;
mod a_kept_alive_show_whose_condition_reruns_unchanged_leaves_its_slot_alone;
mod a_list_rebuilt_under_its_for_each_handle_takes_it_over;
mod a_match_shows_the_first_case_that_holds;
mod a_mock_dom_follows_its_signals_through_show_and_for_each;
mod a_mock_dom_loading_shows_its_fallback_until_what_its_content_holds_arrives;
mod a_mock_dom_row_that_leaves_disposes_the_effects_it_built;
mod a_mock_dom_show_keep_alive_hides_the_element_it_built_and_brings_it_back;
mod a_mock_dom_show_keep_alive_holds_its_childs_updates_while_hidden;
mod a_show_among_a_run_adds_no_host_node;
mod a_show_over_an_option_hands_its_children_a_signal_of_what_is_in_it;
mod a_show_with_a_fallback_fills_a_single_child_slot_and_swaps_it;
mod a_show_with_a_fallback_swaps_runs_of_different_lengths;
mod a_show_without_a_fallback_fills_an_optional_single_slot;
mod a_tracked_read_in_a_component_body_panics_naming_its_type;
mod an_optional_prop_passes_on_to_another_component_as_it_is;
mod components_can_return_and_collect_child_runs;
mod dynamic_and_keyed_show_their_fallback_while_the_value_is_none;
mod for_each_rows_are_runs_that_move_whole_and_keyed_rows_follow_their_item;
mod handlers_effects_and_untracked_reads_in_a_component_body_do_not_panic;
mod modifiers_are_functions_on_built_values;
mod modifiers_run_after_props_and_inside_render_slots;
mod show_can_wrap_a_component_returning_multiple_children;
mod show_keep_alive_retains_multiple_children_and_pauses_nested_updates;

thread_local! {
    static NEXT_ID: Cell<usize> = const { Cell::new(0) };
}

fn fresh_ids() {
    NEXT_ID.with(|next| next.set(0));
}

struct DomElement {
    id: usize,
    tag: &'static str,
    text: RefCell<String>,
    children: RefCell<ChildList<DomNode>>,
}

#[derive(Clone)]
struct DomNode(Rc<DomElement>);

impl DomNode {
    fn new(tag: &'static str) -> Self {
        let id = NEXT_ID.with(|next| next.replace(next.get() + 1));
        Self(Rc::new(DomElement {
            id,
            tag,
            text: RefCell::new(String::new()),
            children: RefCell::new(ChildList::default()),
        }))
    }

    fn html(&self) -> String {
        flush();
        let children: String = self.0.children.borrow().iter().map(DomNode::html).collect();
        let (id, tag) = (self.0.id, self.0.tag);
        format!("<{tag} id={id}>{}{children}</{tag}>", self.0.text.borrow())
    }
}

impl ChildValue for DomNode {}

impl PartialEq for DomNode {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

crate::child_type!(DomNode);

impl SlotChild for DomNode {
    type Stored = DomNode;
    type Node = DomNode;

    fn node(stored: &DomNode) -> DomNode {
        stored.clone()
    }

    fn store(self) -> DomNode {
        self
    }
}

struct DomHost(Weak<DomElement>);

impl DomHost {
    fn with(&self, f: impl FnOnce(&DomElement)) {
        if let Some(element) = self.0.upgrade() {
            f(&element);
        }
    }
}

impl SlotHost<DomNode> for DomHost {
    fn append(&self, stored: DomNode) {
        self.with(|element| element.children.borrow_mut().push(stored));
    }

    fn open(&self) -> SlotId {
        let element = self
            .0
            .upgrade()
            .expect("a slot opens while its element is built");
        element.children.borrow_mut().open()
    }

    fn fill(&self, slot: SlotId, items: Vec<DomNode>) {
        self.with(|element| element.children.borrow_mut().fill(slot, items));
    }
}

impl IntoSlotHost<DomNode> for &DomNode {
    fn into_slot_host(self) -> Rc<dyn SlotHost<DomNode>> {
        Rc::new(DomHost(Rc::downgrade(&self.0)))
    }
}

#[component]
fn Element(
    tag: &'static str,
    #[prop(default = String::new())] text: Prop<String>,
    texts: Option<Signal<usize>>,
    children: Children<DomNode>,
) -> DomNode {
    let element = DomNode::new(tag);
    let shown = Rc::downgrade(&element.0);
    create_effect(move || {
        let text = text.get();
        if let Some(texts) = texts {
            texts.update(|texts| *texts += 1);
        }
        if let Some(shown) = shown.upgrade() {
            *shown.text.borrow_mut() = text;
        }
    });
    children.mount(&element);
    element
}

type Child = DomNode;

/// An element that keeps one child, as a frame does.
#[component]
fn Holder(tag: &'static str, children: Child) -> DomNode {
    let element = DomNode::new(tag);
    element.0.children.borrow_mut().push(children);
    element
}

#[component]
fn MaybeHolder(tag: &'static str, children: Option<Child>) -> DomNode {
    let element = DomNode::new(tag);
    if let Some(child) = children {
        element.0.children.borrow_mut().push(child);
    }
    element
}

/// What a core does with control flow in a slot that takes one node: a host node holding
/// the child it swaps.
fn host(child: crate::reactive::LazyChild<DomNode>) -> DomNode {
    let host = DomNode::new("host");
    let held = Rc::downgrade(&host.0);
    child.mount(move |child| {
        if let Some(held) = held.upgrade() {
            let mut children = held.children.borrow_mut();
            *children = ChildList::default();
            if let Some(child) = child {
                children.push(child);
            }
        }
    });
    host
}

impl crate::reactive::IntoChild<DomNode> for crate::reactive::OneOf<DomNode> {
    fn into_child(self) -> DomNode {
        host(self.into_lazy())
    }
}

impl crate::reactive::AtMostOneChild<DomNode>
    for crate::reactive::SingleChild<crate::reactive::ZeroOrOne<DomNode>>
{
    fn at_most_one_child(self) -> Option<DomNode> {
        Some(host(self.0.into_lazy()))
    }
}

mod modifiers {
    use super::*;

    pub fn suffix(node: DomNode, suffix: &str) -> DomNode {
        node.0.text.borrow_mut().push_str(suffix);
        node
    }

    pub fn wrap(node: DomNode, tag: &'static str) -> DomNode {
        let wrapper = DomNode::new(tag);
        wrapper.0.children.borrow_mut().push(node);
        wrapper
    }

    pub fn stringify(node: DomNode, suffix: &str) -> String {
        format!("{}{suffix}", node.html())
    }
}

#[component]
fn OrdinaryProps(test_id: String, node_ref: String) -> DomNode {
    view! {
        <Element tag="p" text={format!("{test_id}:{node_ref}")} />
    }
}
mod two_lists_given_one_for_each_handle_is_a_mistake;
