# Dock defaults: a layout in a few lines

Depends on: none.

Eleven editors (audio, image, pdf, the game module, the game, map, logic-grid, database twice,
the canvas and pan-zoom) lay out a stage beside a side panel, and guides/apps/editor.md
("Chrome and side panels") tells new apps to copy the recipe. The audio app's:

```
<Docking layout focus={Some(STAGE)}>
    <DockSplit id="audio" size={SplitSize::Second(PANEL_WIDTH)}>
        <DockPane id="stage" locked=true bare=true>
            <DockTab id=STAGE title="Track">…</DockTab>
        </DockPane>
        <Show value={chrome}>
            <DockPane id="panel" locked=true>
                <DockTab id=PANEL title="Audio">
                    <Frame fill=Surface><Scroll><Frame padding><List>…
```

Rather than a component for this one shape, the dock's defaults make every layout this short
to write, so a stage and a side panel is about
`<DockSplit><DockPane stage>…</DockPane><DockPane chrome><DockTab title="Audio">…</DockTab></DockPane></DockSplit>`.

## Steps

1. **Ids are derived.** A split's and a pane's id default to their position under their parent;
   a tab's to its title's place among its siblings. An explicit id stays for layouts that are
   saved and reordered by the user.
2. **Panes in an editor's dock are locked by default.** `movable` opts out; the workspace's own
   dock keeps its panes movable.
3. **`stage` marks the stage pane.** It is bare (no tab bar), holds one untitled tab, takes the
   dock's initial focus (`focus` goes), and is what `DockState::stage_of` finds in the gathered
   mode.
4. **`chrome` marks a pane the frame hides** with the rest of the chrome, so
   `<Show value={chrome_shown()}>` around panels goes.
5. **A non-bare tab's body is the standard panel.** Surface fill, a `Scroll` filling the panel,
   padding inside it, then the content (`scroll(padding(content))`). `bare` on a tab opts out.
6. **Sizes have defaults.** A split with a stage gives the other side the side panel width; a
   size is given only to differ.
7. **`DockingLayout` is optional.** `Docking` makes one when none is passed; an editor that
   reads the mode or saves sizes passes its own.
8. **The editors and the guide follow.** The eleven editors drop what the defaults now say, and
   guides/apps/editor.md and guides/beui/docking.md show the short form.

## The test

- Each editor's paintings are unchanged after step 8.
- A beui test of a dock written with only `stage`, `chrome` and titles builds the same tree as
  the long form.
