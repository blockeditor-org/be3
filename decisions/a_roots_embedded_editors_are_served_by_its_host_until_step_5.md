# A root's embedded editors are served by its host until step 5

Until step 5 moves the block stack into the root, the host opens and serves the editors of the
apps a root guest embeds. The root runs each app: its hello, pacing, screens, input and presents.
The host serves what the app asks under those editors: block calls, host services, the network,
debug and the published interfaces. The root asks its host for a block's editor
(`be3:root`'s `host.editor-of`), spawns that app, and has the host open the editor in it
(`host.open-editor`).

Everything after that crosses `be3:root` as be-wasm-ops frames, each as it was encoded:
- `host.forward` carries what the app sent on the editor's ops and on the ops it started under them;
- `host.forwarded` carries what the host answers, which the root sends the app unread.

The host's end is be-host-runtime's `Forwarded`: a relayed `Wire` and `Instances`, with the
app's manifest's rules, as a `Runtime` serves an app of its own. Its connection starts ops with
be-wasm-ops' `RELAYED_BIT`, so they never meet the root's own ops on the same app. The root
therefore routes by op id alone (be-sdk-app-host's `Wire::take_forwarded` and
`Wire::relay_forwarded`). Step 5 deletes the host's end (plans/host_as_platform_adapter.md,
4g7b1).

## Why

- be3:root stays independent of any GUI framework and of the app protocol's types. It carries
  the protocol's own frames and ids, which any root, beui or not, passes on without reading
  them.
- The host already serves editors with every per-app rule: grants, network hosts, block agents,
  sharing, local ids in checkouts, and access. Forwarding reuses that rather than copying it into
  the root before the stack is there to serve it.
- The routing is structural. Whatever the app starts under a host editor is the host's,
  whatever the interface, so a call added to the app protocol needs no change to the root.
- The host opens the editor, so the opening is the host's own: the instance id, the role, the
  translated block ids, `editable`. Events the host has for the editor (`SetEditable`,
  `Hosting`, the focus) reach it on the same relayed ops.

## Rejected

- **Pulling step 5 forward** (the block stack as an instance the root spawns, so the root serves
  its apps itself): the stack's move is the largest step left, with its own memory, workers and
  smaller updates. Doing it first would hold up the root's views (4g7b to 4g7g) behind it.
- **Editors without content until step 5** (the root embeds apps but refuses their block calls):
  nothing the root shows would be usable, and the host tests that compare the root with
  be-host's own UI could not run until step 5.
- **Forwarding decoded calls (`Heard`) or a call per interface**: either puts the app
  protocol's types, or a list of its interfaces, into be3:root, which every change to the app
  protocol would have to follow.
