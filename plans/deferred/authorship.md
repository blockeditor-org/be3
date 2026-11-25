# Authorship: set aside

Depends on: none.

Set aside until the user picks them up. What is built is in guides/blocks/keys.md ("Signing
what a person writes", "Forks").

- **First use of a member's key is trusted.** A member who is not a contact has
  the key the server named the first time, so a dishonest server can name one it
  holds then. A contact's key is never taken from the server. Set aside by the owner.
- **Forks across devices, through contact letters.** Heads of signed logs are compared
  through sessions, commits and the account's block (guides/blocks/keys.md, "Forks"); a
  contact's letter could carry the heads of the logs of blocks both accounts hold, capped at
  the most recently read, so a letter stays bounded.
- **A replaced key and someone holding the old phrase** look alike: an entry
  signed with a key the account replaced, arriving after this device learned of
  the new one, shows as "sent with an old key", not forged, since the account's
  own devices not yet told of the new phrase sign with the old key too. Telling
  them apart would need the time the account took the new phrase, signed in its
  key change, and a grace period for its devices.
