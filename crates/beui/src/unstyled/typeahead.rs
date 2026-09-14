use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(crate) struct Typeahead {
    search: String,
    typed_at: Option<Instant>,
}

impl Typeahead {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn matched(
        &mut self,
        typed: &str,
        from: usize,
        count: usize,
        label: impl Fn(usize) -> String,
    ) -> Option<usize> {
        if count == 0 || typed.is_empty() || typed.chars().any(char::is_control) || typed == " " {
            return None;
        }
        let typed = typed.to_lowercase();
        let now = Instant::now();
        if self
            .typed_at
            .is_none_or(|last| now.duration_since(last) > TIMEOUT)
        {
            self.search.clear();
        }
        self.typed_at = Some(now);
        self.search.push_str(&typed);
        let search = self.search.clone();
        let repeated = search.chars().all(|letter| search.starts_with(letter));
        let prefix = if repeated { &typed } else { &search };
        let start = if repeated || search == typed {
            from + 1
        } else {
            from
        };
        (0..count)
            .map(|offset| (start + offset) % count)
            .find(|candidate| {
                label(*candidate)
                    .to_lowercase()
                    .starts_with(prefix.as_str())
            })
    }
}
