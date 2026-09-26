# Chess

## Players

Two: White and Black. The first player to make a move plays White; the first
other player to move after them plays Black.

## The board and the pieces

An 8 by 8 board of light and dark squares, with a dark square in each player's
left-hand corner. Each player has a king, a queen, two rooks, two bishops, two
knights and eight pawns. The pieces start on the player's own back rank - rook,
knight, bishop, queen, king, bishop, knight, rook - with the pawns in front of
them.

## A turn

White moves first, then the players take turns. On your turn you move one of
your pieces, capturing the enemy piece on the square it lands on, if any.

- The king steps one square in any direction.
- The rook moves any distance along a rank or file, the bishop any distance
  along a diagonal, and the queen either way. None of them may jump.
- The knight jumps in an L: two squares one way and one the other.
- The pawn steps one square forward, or two from where it started, and
  captures one square diagonally forward. A pawn that steps two squares past
  an enemy pawn's capture may be taken in passing on the very next move, as if
  it had stepped one. A pawn reaching the far rank becomes a queen, rook,
  bishop or knight.
- A king and rook that have not moved may castle, when nothing stands between
  them: the king steps two squares toward the rook and the rook lands on the
  square the king crossed. A king may not castle out of, through or into check.

You may never make a move that leaves your own king attacked (in check).

## How it ends

- Checkmate: the player to move is in check and has no move. They lose.
- Stalemate: the player to move is not in check and has no move. A draw.
- A draw after fifty moves each with no capture and no pawn move, or when the
  same position comes round for the third time.
- Either player may resign at any time.

## Writing it down

Moves are written in standard algebraic notation: the piece's letter (none for
a pawn), `x` for a capture, and the square it lands on - `Nf3`, `exd5`,
`O-O`, `e8=Q`. A check is marked `+` and checkmate `#`.
