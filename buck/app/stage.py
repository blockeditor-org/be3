#!/usr/bin/env python3
#
# Lays out the app the way it runs: the executable, and beside it every
# plugin's manifest, renamed after the plugin's id, and the module the manifest
# names as its entry point. That is what the app's native plugin discovery
# scans the executable's own directory for, and what ./scripts/build produces
# under cargo. A module can be followed by the .cwasm compiled from it, which
# wasmtime maps in rather than compiling the module at launch, and which goes
# beside it.
#
# Usage:
#   stage.py OUT EXECUTABLE (MANIFEST=MODULE [--artifact=CWASM])...

import json
import os
import shutil
import sys

out, executable, *arguments = sys.argv[1:]

os.makedirs(out)
shutil.copy2(executable, os.path.join(out, os.path.basename(executable)))

for argument in arguments:
    if argument.startswith("--artifact="):
        artifact = argument.removeprefix("--artifact=")
        shutil.copyfile(artifact, staged.removesuffix(".wasm") + ".cwasm")
        continue
    manifest, module = argument.split("=", 1)
    with open(manifest) as file:
        fields = json.load(file)
    if os.path.basename(module) != fields["entry_point"]:
        sys.exit("{} names {} as its entry point, but its module is {}".format(manifest, fields["entry_point"], module))
    shutil.copyfile(manifest, os.path.join(out, fields["id"] + ".plugin.json"))
    staged = os.path.join(out, fields["entry_point"])
    shutil.copyfile(module, staged)
