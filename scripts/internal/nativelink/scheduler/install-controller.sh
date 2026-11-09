#!/bin/bash
# Run as root on the scheduler after setup-scheduler.sh. Usage: install-controller.sh
# The controller creates and manages workers with the boat key the nl-scheduler environment delivers as the secret file
# ~/nl-controller.key: limited to the nl-worker environment, actions sandbox.create, sandbox.read, sandbox.update,
# sandbox.delete-own and account.read (so it can read `boat limits`). Run it again after a new key is delivered.
set -euo pipefail; cd "$(dirname "$(readlink -f "$0")")"
key=/home/user/nl-controller.key
[ -s "$key" ] || { echo "no $key: set it with boat env set-file nl-scheduler nl-controller.key" >&2; exit 1; }
chmod 600 "$key"
install -m0755 nl-controller.py /usr/local/bin/nl-controller.py
install -d /opt/nl/worker
install -m0644 ../worker/worker-direct.json5 ../worker/nativelink-worker-sch.service ../worker/nl-load.py ../worker/nl-load.service /opt/nl/worker/
install -m0755 ../worker/worker-bootstrap.sh /opt/nl/worker/
install -d -m700 /var/lib/nl-controller/sk-home
tr -d '\n' < "$key" | env HOME=/var/lib/nl-controller/sk-home BOAT_NO_UPDATE=1 boat login --key-stdin > /dev/null
install -m0644 nl-controller.service /etc/systemd/system/nl-controller.service
systemctl daemon-reload; systemctl enable nl-controller; systemctl restart nl-controller
sleep 3; systemctl is-active nl-controller
