# An HTML core for beui on the web: what is left

Depends on: none.

beui on the web runs an HTML core (`beui-core-html`), switched in at compile
time by `--cfg beui_html`; guides/beui/core.md (Cores) describes it, and
decisions/beui_has_an_html_core_on_the_web.md says why. What the HTML core
lacks, each with what "done" looks like. Larger directions and the rest of the
ignored props are in plans/deferred/html_core.md.

- **Tests for `TreeNode`'s selection hooks.** On HTML `set_text_selection`
  draws through the text's CSS highlights and `set_text_selection_handles` as
  caret handles in its decoration overlay (`beui-core-html/src/text.rs`), but
  nothing shows either: drive reads neither core's highlight, and a
  server-rendered painting runs no layout. Done: a test per hook on the HTML
  core, such as a drive reading of a text's highlighted ranges on both cores.
