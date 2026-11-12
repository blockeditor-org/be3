# An HTML core for beui on the web: what is left

Depends on: none.

beui on the web runs an HTML core (`beui-core-html`), switched in at compile
time by `--cfg beui_html`; guides/beui/core.md (Cores) describes it, and
decisions/beui_has_an_html_core_on_the_web.md says why. What the HTML core
lacks, each with what "done" looks like. Larger directions and the rest of the
ignored props are in plans/deferred/html_core.md.

- **`Sheet`'s stops as CSS scroll snap.** `Sheet` still drives its `Offset`
  itself on HTML, as natively. Done: on the HTML core a sheet's stops are scroll
  snap points of its scroll element, and a sheet released between stops
  settles on one by the browser's snapping, with no beui animation.
- **Tests for `TreeNode`'s selection hooks.** On HTML `set_text_selection`
  draws through the text's CSS highlights and `set_text_selection_handles` as
  caret handles in its decoration overlay (`beui-core-html/src/text.rs`), but
  nothing shows either: drive reads neither core's highlight, and a
  server-rendered painting runs no layout. Done: a test per hook on the HTML
  core, such as a drive reading of a text's highlighted ranges on both cores.
- **A held clip.** The HTML core's `copy` (`page.rs`) writes text alone and keeps
  nothing, so a rich clip copied and pasted within the page loses all but its
  text. Done: it keeps a `HeldClip`, as native platforms do
  (guides/beui/platforms.md), and a paste whose text matches the held clip's
  answers be3 formats from it.
