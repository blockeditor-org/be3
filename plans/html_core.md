# An HTML core for beui on the web: what is left

Depends on: none.

beui on the web runs an HTML core (`beui-core-html`), switched in at compile
time by `--cfg beui_html`; guides/beui/core.md (Cores) describes it, and
decisions/beui_has_an_html_core_on_the_web.md says why. What the HTML core
lacks, each with what "done" looks like. Larger directions and the rest of the
ignored props are in plans/deferred/html_core.md.

- **A held clip.** The HTML core's `copy` (`page.rs`) writes text alone and keeps
  nothing, so a rich clip copied and pasted within the page loses all but its
  text. Done: it keeps a `HeldClip`, as native platforms do
  (guides/beui/platforms.md), and a paste whose text matches the held clip's
  answers be3 formats from it.
