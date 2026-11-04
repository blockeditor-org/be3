# The text area's completion menu does not use SearchList

`unstyled::SearchList<K>` is the one searchable list under `Select` and `CommandPalette` (and so the desktop app's launcher): it owns the query field, ranking (`rank`, `substring_score`, `fuzzy_score`), highlight, keys, scroll-to-row, Enter, clicks and hover. The text area's completion menu (`text_area/completion.rs`) keeps its own list.

## Why

The completion menu's query is a span of the text area's own text, not a field of its own; its keys reach the text area's key map while the caret stays in the document (and Tab completes); its overlay passes input through instead of trapping focus; and its `Completer` returns matches already ordered. Sharing would mean splitting the search list's highlight and keys away from its field for one user that has no scroll, no disabled rows and no Page keys.
