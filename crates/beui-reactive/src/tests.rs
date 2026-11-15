use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use crate::create_linked_signal;
use crate::{
    Get, KeyedItems, OwnedKeyedStore, OwnedStored, RootScope, Scope, Set, Store, create_effect,
    create_memo, create_selector, create_signal, flush, on_cleanup, owner_scope, store, untrack,
};

mod a_binding_inside_an_effect_tracks_for_itself;
mod a_binding_reading_a_memo_whose_scope_went_waits_for_the_flush_that_disposes_it;
mod a_binding_reads_the_context_of_the_scope_it_runs_in;
mod a_binding_runs_at_once_and_again_only_for_what_it_read;
mod a_binding_that_reads_nothing_leaves_nothing_to_run;
mod a_boundary_reports_a_failure_and_forgets_reads_whose_scope_went;
mod a_boundary_waits_for_the_pending_values_read_under_it;
mod a_child_scope_lives_as_long_as_the_scope_it_was_opened_in;
mod a_context_reaches_the_effects_a_nested_scope_creates;
mod a_detached_scope_reads_context_without_its_parent_disposing_it;
mod a_diamond_over_a_pending_source_is_pending_until_it_settles_and_wakes_once;
mod a_failure_goes_to_the_errored_above_it_whichever_way_it_nests_with_a_loading;
mod a_failure_read_under_an_error_boundary_is_caught_and_reset_asks_again;
mod a_failure_with_no_errored_above_it_shows_in_an_unrevealed_loading;
mod a_held_value_keeps_what_it_showed_while_its_source_reloads;
mod a_keyed_mapping_builds_a_row_for_each_repeat_of_a_key;
mod a_keyed_mapping_builds_reuses_and_disposes_its_items;
mod a_keyed_mapping_extended_at_its_end_builds_only_the_new_keys;
mod a_keyed_row_kept_across_a_rerun_still_reads_context;
mod a_keyed_store_filters_its_keys_waking_only_when_one_joins_or_leaves;
mod a_keyed_store_forgets_items_whose_keys_are_gone;
mod a_keyed_store_frees_the_signals_of_keys_that_are_gone;
mod a_keyed_store_only_wakes_the_item_that_changed;
mod a_keyed_store_sets_and_removes_one_item_at_a_time;
mod a_keyed_store_sorts_its_keys_moving_only_the_item_that_changed;
mod a_keyed_store_sums_its_items_by_applying_each_difference;
mod a_keyed_store_tracks_its_largest_and_smallest_item;
mod a_keyed_store_updates_one_item_in_place;
mod a_keyed_store_view_is_freed_with_the_scope_that_made_it;
mod a_keyed_store_view_rebuilds_once_when_a_signal_it_reads_changes;
mod a_linked_signal_can_keep_part_of_what_was_written_when_it_resets;
mod a_linked_signal_holds_writes_until_its_source_changes;
mod a_linked_signal_keeps_a_draft_while_its_source_returns_the_same;
mod a_memo_can_compute_from_the_value_it_computed_last;
mod a_memo_of_a_memo_over_a_pending_source_holds_a_loading;
mod a_memo_over_a_pending_value_is_pending_until_it_settles;
mod a_memo_that_panics_on_its_first_run_cleans_up_what_it_made;
mod a_memo_that_reads_nothing_keeps_what_it_made_until_its_scope_goes;
mod a_nested_pausable_scope_waits_while_any_scope_around_it_is_paused;
mod a_panicking_effect_can_run_again;
mod a_paused_scope_defers_its_effects_until_it_resumes;
mod a_reader_reads_a_signal_a_memo_or_a_closure_alike;
mod a_rerun_async_memo_drops_its_last_future_and_keeps_its_last_answer;
mod a_reset_asks_again_only_what_failed_under_its_errored;
mod a_resumed_effect_runs_when_what_it_read_changed_and_changed_back;
mod a_scope_opened_inside_an_effect_lives_until_its_handle_drops;
mod a_scope_pausing_effects_still_runs_its_bindings;
mod a_selector_forgets_keys_whose_watchers_were_disposed;
mod a_selector_memo_tracks_one_key_without_watching_the_source;
mod a_selector_only_reruns_the_keys_that_gained_or_lost_selection;
mod a_store_wakes_only_the_field_that_changed;
mod a_stored_value_is_shared_by_its_copies_and_freed_with_its_scope;
mod a_trackable_follows_a_trigger_or_a_value_alike;
mod a_trigger_wakes_what_tracked_it_each_time_it_fires;
mod a_write_outside_an_entry_wakes_the_host_once;
mod a_zone_is_woken_once_when_its_first_effect_waits;
mod all_cleanups_run_even_when_one_panics;
mod an_answer_for_an_old_memo_value_is_dropped_and_one_for_an_equal_value_lands;
mod a_memo_over_an_async_memo_reads_its_answer_for_the_current_input;
mod an_answer_there_at_once_is_for_the_latest_write;
mod an_answer_there_at_once_under_a_loading_reveals_it_in_the_same_flush;
mod an_async_memo_whose_source_reads_itself_is_a_cycle;
mod reading_an_async_memo_and_its_pending_asks_once;
mod an_answer_landing_after_its_async_memo_is_due_again_is_dropped;
mod an_async_memo_in_a_paused_scope_asks_only_where_bindings_run;
mod an_async_memo_reads_loading_then_its_answer;
mod an_effect_keeps_its_zone_for_the_effects_it_creates;
mod an_effect_made_outside_every_zone_waits_until_the_zone_entered_is_left;
mod an_effect_on_change_runs_only_after_its_dependencies_change;
mod an_effect_on_its_dependencies_runs_untracked_with_their_last_value;
mod an_effect_reading_many_signals_again_and_again_runs_once_per_change;
mod an_effect_that_reads_nothing_is_discarded_after_its_first_run;
mod an_effect_waits_for_its_zone_while_another_zone_is_entered;
mod an_effect_waits_for_its_zone_while_no_zone_is_entered;
mod an_effect_with_cleanup_is_kept_without_dependencies;
mod an_effect_woken_in_an_inner_zone_runs_when_its_own_zone_is_returned_to;
mod an_inner_boundary_keeps_its_reads_from_the_outer_one;
mod cleanup_precedes_next_execution;
mod cleanup_reads_do_not_become_dependencies;
mod conditional_dependencies_are_replaced;
mod conditional_memos_replace_dependencies;
mod diamond_and_direct_dependencies_never_glitch;
mod disposal_releases_captured_resources;
mod disposing_a_scope_frees_what_it_owns;
mod disposing_an_async_memo_in_flight_cancels_its_future;
mod downstream_effect_recovers_after_memo_panics;
mod dropping_scope_cancels_queued_effects;
mod effects_can_write_signals_and_settle_without_recursion;
mod flushing_a_scope_runs_only_the_effects_it_owns;
mod memo_panic_can_be_retried;
mod memo_writes_are_rejected_even_when_untracked;
mod memos_are_lazy_cached_and_filter_equal_outputs;
mod nested_effects_are_disposed_before_parent_reruns;
mod non_clone_values_can_be_read_and_updated;
mod opening_a_child_scope_outside_any_scope_panics;
mod panic_restores_tracking_and_deferral;
mod panicking_update_still_invalidates_mutated_value;
mod reconciling_an_unchanged_list_wakes_nothing;
mod reordering_a_keyed_store_keeps_the_item_signals;
mod repeated_reads_only_subscribe_once;
mod runaway_effects_are_stopped_and_scheduler_remains_usable;
mod scopes_dispose_effects_and_run_cleanup_once;
mod selector_reads_are_current_before_a_flush;
mod settling_alone_wakes_a_reader_whose_value_stays_equal;
mod signals_track_changes_and_skip_equal_sets;
mod the_nearest_context_shadows_the_ones_above_it;
mod the_write_that_pauses_a_scope_still_reaches_its_effects;
mod untracked_memo_reads_still_refresh_without_subscribing;
mod untracked_reads_do_not_subscribe;
mod updating_a_store_wakes_only_the_fields_it_changed;
mod writes_defer_effects_until_a_flush_but_memo_reads_are_current;

/// A test's executor for async memos: it holds what they spawn, and `run_futures` polls each
/// until none can go further.
mod executor {
    use std::cell::RefCell;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    use crate::LocalFuture;

    thread_local! {
        static SPAWNED: RefCell<Vec<LocalFuture>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn install() {
        crate::spawn_with(|future| {
            SPAWNED.with(|spawned| spawned.borrow_mut().push(future));
            Ok(())
        });
    }

    /// Polls every spawned future once more, dropping those that finished, and flushes.
    pub(super) fn run_futures() {
        let mut held = SPAWNED.with(|spawned| std::mem::take(&mut *spawned.borrow_mut()));
        let mut context = Context::from_waker(Waker::noop());
        held.retain_mut(|future| future.as_mut().poll(&mut context).is_pending());
        SPAWNED.with(|spawned| {
            let mut spawned = spawned.borrow_mut();
            held.append(&mut spawned);
            *spawned = held;
        });
        crate::flush();
    }

    pub(super) fn spawned() -> usize {
        SPAWNED.with(|spawned| spawned.borrow().len())
    }

    /// A future that waits until its test answers it.
    pub(super) struct Answer<T>(Rc<RefCell<Option<T>>>);

    impl<T> Clone for Answer<T> {
        fn clone(&self) -> Self {
            Self(Rc::clone(&self.0))
        }
    }

    impl<T> Default for Answer<T> {
        fn default() -> Self {
            Self(Rc::default())
        }
    }

    impl<T> Answer<T> {
        pub(super) fn give(&self, value: T) {
            self.0.replace(Some(value));
        }
    }

    impl<T> Future for Answer<T> {
        type Output = T;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<T> {
            match self.0.borrow_mut().take() {
                Some(value) => Poll::Ready(value),
                None => Poll::Pending,
            }
        }
    }
}
