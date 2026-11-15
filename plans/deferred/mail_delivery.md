# Mail delivery

Depends on: none.

Set aside until the user picks it up.

be-server sends mail through `mail::Delivery`, whose only implementation,
`LogDelivery`, writes each message to the server log, the hosted server's
included. Since accounts sign in by emailed code (guides/blocks/server.md, "Accounts"), that log
holds every live sign-in code and link, and the operator reads it (or runs
`be-server login-link`) whenever someone signs in a device. The user judged
that only a little worse than before, when the log already held every
verification and reset link, so real delivery waits.

## Steps

1. A `mail::Delivery` that sends, through a provider's API or an SMTP relay
   (which one is the user's choice), configured on the command line.
2. Its credentials in a file under the data directory's `secrets/` on the
   hosted server, as `gateway.env` is (servers/api/default.nix), passed with
   `EnvironmentFile` or `LoadCredential`; never in the repository.
3. SPF, DKIM and DMARC records for the sending domain.
4. guides/hosting.md: how an operator configures delivery, and that the log no
   longer holds codes once it is on.

Done: a test where the sending delivery talks to a fake SMTP server (or
provider endpoint) and the message arrives with its headers; the hosted server
sends a real sign-in mail.
