# Push endpoints are allowlisted

be-server sends Web Push only to endpoints on the known push services, by exact host or fixed
suffix (`push::PUSH_HOSTS`, `push::PUSH_HOST_SUFFIXES`): FCM (`fcm.googleapis.com`, Chrome),
Mozilla autopush (`updates.push.services.mozilla.com` and `*.push.services.mozilla.com`,
Firefox), Apple (`web.push.apple.com`, Safari) and Microsoft WNS (`*.notify.windows.com`,
Edge). The operator adds hosts with `--push-host HOST`, for a self-hosted UnifiedPush server
such as ntfy. Anything else is refused at registration and dropped at send. The public-address,
https and port 443 checks stay as well.

## Why

A device names its endpoint, so with any host allowed, the members of any organization, an
attacker's own included, could make the server POST VAPID-signed requests to a third party at
the notify rate, times the recipients, times each one's endpoints: an open POST relay with the
server's address and key on it. The known services are few and stable, and every browser
subscribes on one of them.

## Rejected

- Any public host: the relay above.
- Verifying an endpoint by a challenge before using it: the standard push services do not
  answer one, so it would refuse every browser.
- Sending through a push gateway of our own: it only moves the relay to the gateway.
