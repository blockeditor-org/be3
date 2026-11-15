use be_sdk_beui::beui::reactive::*;

/// The stored value, as a list of one, while no option of its select holds
/// it, and empty otherwise.
///
/// A settings select lists it after its options, as OS settings do, rather than
/// showing nothing chosen: a number as "Custom (…)", and what has no name to show
/// by what became of it ("Unknown editor", "Disconnected display"). Choosing another option stores that
/// one, which drops it from the list.
pub(crate) fn unlisted<T>(
    stored: Memo<Option<T>>,
    listed: impl Fn(&T) -> bool + 'static,
) -> Memo<Vec<T>>
where
    T: Clone + PartialEq + 'static,
{
    create_memo(move || {
        stored.with(|stored| {
            stored
                .iter()
                .filter(|value| !listed(value))
                .cloned()
                .collect()
        })
    })
}
