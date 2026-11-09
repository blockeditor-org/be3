#!/bin/bash
# Run as root on the scheduler after setup-scheduler.sh. Usage: install-controller.sh BOAT_WORKERS_KEY_FILE
# The key is the boat key the controller creates and manages workers with: limited to the nl-worker environment, actions
# sandbox.create, sandbox.read, sandbox.update, sandbox.delete-own (add account.read so it can read `boat limits`).
set -euo pipefail; cd "$(dirname "$(readlink -f "$0")")"
key=${1:?usage: install-controller.sh BOAT_WORKERS_KEY_FILE}
install -m0755 nl-controller.py /usr/local/bin/nl-controller.py
install -d /opt/nl/worker
install -m0644 ../worker/worker-direct.json5 ../worker/nativelink-worker-sch.service ../worker/nl-load.py ../worker/nl-load.service /opt/nl/worker/
install -m0755 ../worker/worker-bootstrap.sh /opt/nl/worker/
install -d -m700 /var/lib/nl-controller/sk-home
install -m0600 "$key" /etc/nativelink/boat-workers.key
tr -d '\n' < /etc/nativelink/boat-workers.key | env HOME=/var/lib/nl-controller/sk-home BOAT_NO_UPDATE=1 boat login --key-stdin > /dev/null
install -m0644 nl-controller.service /etc/systemd/system/nl-controller.service
systemctl daemon-reload; systemctl enable --now nl-controller
sleep 3; systemctl is-active nl-controller
