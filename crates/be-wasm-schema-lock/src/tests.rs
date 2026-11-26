use super::*;

fn lock_of(files: &[(&str, &str)], old: &Lock) -> Result<Lock, Vec<String>> {
    let sources: Vec<Source> = files
        .iter()
        .map(|(path, text)| Source { path, text })
        .collect();
    update(&sources, old)
}

fn ids(lock: &Lock, name: &str) -> Vec<(u32, String, bool)> {
    lock.types[name]
        .members
        .iter()
        .map(|member| (member.id, member.name.clone(), member.retired))
        .collect()
}

fn one_error(result: Result<Lock, Vec<String>>, containing: &str) {
    let errors = result.expect_err("the change is refused");
    assert!(
        errors.iter().any(|error| error.contains(containing)),
        "no error mentions {containing:?}: {errors:#?}"
    );
}

const PERSON: &str =
    "#[derive(Schema)] #[schema(unfrozen)] struct Person { name: String, age: u32 }";

const FROZEN_PERSON: &str =
    "#[derive(Schema)] #[schema(frozen)] struct Person { name: String, age: u32 }";

fn frozen_person() -> Lock {
    lock_of(&[("a.rs", FROZEN_PERSON)], &Lock::default()).unwrap()
}

mod a_frozen_enum_says_whether_it_may_grow;
mod a_frozen_member_is_removed_only_once_the_lockfile_retires_it;
mod a_frozen_member_may_be_moved;
mod a_frozen_struct_keeps_keeping_unknown_fields;
mod a_frozen_type_never_gives_a_retired_name_again;
mod a_frozen_type_refuses_what_changes_its_bytes;
mod a_frozen_type_stays_frozen;
mod a_new_type_is_numbered_in_source_order;
mod a_removed_member_is_retired_with_its_type_and_its_id_never_given_again;
mod a_rename_of_a_frozen_member_is_made_in_the_lockfile;
mod a_schema_type_has_one_name_in_its_crate;
mod an_unfrozen_type_takes_any_change;
mod every_type_says_whether_it_is_frozen;
mod freezing_records_where_later_members_start;
mod generic_types_are_numbered_too;
mod reordering_keeps_every_id;
mod the_derive_numbers_as_the_next_lockfile_will;
mod the_derive_refuses_what_the_lockfile_froze;
mod the_lockfile_reads_back_as_it_was_written;
mod what_the_format_cannot_number_is_refused;
