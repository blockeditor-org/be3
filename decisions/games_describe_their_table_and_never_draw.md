# A game describes its table and never draws

A deterministic game module returns a `Scene`: a sentence for the viewer and a `Board` saying what is on the table and where, as sprites such as a `Piece` of a kind and seat or a `Card`. The Game block's editor decides what each thing looks like and draws it (guides/adding_a_game.md, What the player sees).

## Why

It is what lets a player pick their own skin for the pieces. The editor also scales the table to fit, slides an item that moved, highlights the spots a move can start from, and lets players mark the board up, none of which the game has to know about.
