# Colours that follow light and dark are references to declared colour variables

A `beui::Color` is a literal `Color32` or a reference to a `ColorVar`, declared with `beui::color_var!` (identity is the declaring module path and item name, never a string) and defined per `ColorScheme` with `define_colors`. The document's theme is made of such references. Code picks a reference whose definitions differ per scheme rather than reading the theme kind or the scheme to choose a literal colour.

## Why

A page prerendered on the server must show the browser's scheme before any script runs and hydrate without changing a colour. The HTML core writes `var(--NAME)` and puts both schemes' values in a `<style>` (dark under `prefers-color-scheme: dark`), so the browser picks; the server cannot know the browser's scheme, so code that branches on it renders differently on each side. Declaring variables by item rather than by string means a variable is reached only by importing it, two crates cannot collide, and its CSS name (a hash of the path) is the same in every build, which hydration needs.

## Rejected

- Choosing a colour by reading the theme's kind or the scheme: the server and the browser can disagree, so the prerendered page shows the wrong colours.
- Naming colour variables by string: two crates could collide.
