# Anonymous share links

Depends on: none (share links are built: guides/blocks/keys.md, "Share links").

Set aside until the user picks it up.

A third choice of who may open a share link: anyone at all, without an
account. Opening one makes an account on the server on the spot (not a local
account, which has no server; guides/hosting.md, Local accounts and the default server), kept on that device
alone with no recovery phrase shown, which signs its grant like any other
opener's and is a guest of the block's organization
(guides/blocks/keys.md, "Share links", Guests).

Open questions before building it:

- Registering with no identity (guides/blocks/server.md, "Accounts"): what the server asks of
  such an account, and how it limits how many one person makes.
- What happens to the account when its device is lost, since nobody saved its
  phrase: it ends with its last grant, or it can be claimed later.

Done: a test where an anonymous opener edits through a link that allows it,
and the organization policy that refuses anonymous openers is enforced.
