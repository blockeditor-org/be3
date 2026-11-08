# Text: later

Depends on: none.

Set aside until the user picks them up.

- **Splices for other editors.** Edits are reported through `Touched`
  (`be-block-model`: `Everything`, `Field` and `Subtree`); editors other than
  the text block could be told the splice too. No editor needs it yet.
- **Inline text fields.** Text inside other models (titles, cells) is a
  `String` register, which keeps ours on an offline merge and counts a
  conflict. A register that keeps both sides of a conflict is wanted, for the
  version-control case where only one side is right (a password).
