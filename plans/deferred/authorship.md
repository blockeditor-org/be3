# Authorship: set aside

Depends on: none.

Set aside from plans/authorship.md until the user picks them up.

- **First use of a member's key is trusted.** A member who is not a contact has
  the key the server named the first time, so a dishonest server can name one it
  holds then. A contact's key is never taken from the server. Set aside by the owner.
- **A server or participant can drop or reorder.** A deleted or withheld message
  shows only as a gap in the `seen` hashes of its author's later messages;
  nothing checks them yet. Set aside by the owner.
