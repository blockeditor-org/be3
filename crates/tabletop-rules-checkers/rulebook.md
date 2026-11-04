# Checkers

## Players

Two: Dark and Light. The first player to make a move plays Dark; the first
other player to move after them plays Light.

## The board and the pieces

An 8 by 8 board, played on its dark squares only. Each player starts with
twelve men on the dark squares of the three rows nearest them.

## A turn

Dark moves first, then the players take turns.

- A man steps one square diagonally forward onto an empty square.
- A man captures by jumping diagonally forward over an enemy piece onto the
  empty square beyond it, removing the piece it jumped. If it can jump again
  from where it lands, it must, and the whole chain is one move.
- If you can capture, you must: when any capture is possible, only captures
  may be played.
- A man reaching the far row is crowned and becomes a king, which ends the
  move. A king steps and jumps diagonally in all four directions.

## How it ends

- A player with no move on their turn - because they have no pieces left, or
  every one is blocked - loses.
- A draw after forty moves each with no capture and no man moving.
- A draw when the same position comes round for the third time.
- Either player may resign at any time.

## Writing it down

The dark squares are numbered 1 to 32, starting from Dark's side, four to a
row. A step is written `11-15`, and a capture lists every square the piece
lands on: `15x22`, or `15x22x29` for a double jump.
