# The text editor parses markdown in small windows, not incrementally

The text editor highlights markdown by parsing windows of about 1 KB plus the way to the nearest blank line, kept outside fences. An edit drops only the windows on the lines it touched (from the document's log of changed ranges, `Document::changes_since`) and shifts the rest.

## Why

tree-sitter-md gets nothing from an old tree: re-parsing unchanged text costs as much as a fresh parse, because its external scanners produce nearly every token. So the cost of an edit is the size of the window parsed, and windows are kept small instead.

## Rejected

- Incremental tree-sitter parsing of the whole document.
