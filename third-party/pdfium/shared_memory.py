import os
import sys


def leb(data, at):
    value = shift = 0
    while True:
        byte = data[at]
        at += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if byte < 0x80:
            return value, at


def encode(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def allow_shared_memory(module):
    out = bytearray(module[:8])
    at = 8
    while at < len(module):
        start = at
        section = module[at]
        at += 1
        size, at = leb(module, at)
        end = at + size
        if section == 0:
            length, cursor = leb(module, at)
            name = module[cursor : cursor + length]
            if name == b"target_features":
                cursor += length
                count, cursor = leb(module, cursor)
                kept = []
                for _ in range(count):
                    prefix = module[cursor]
                    cursor += 1
                    feature_length, cursor = leb(module, cursor)
                    feature = module[cursor : cursor + feature_length]
                    cursor += feature_length
                    if (prefix, feature) != (ord("-"), b"shared-mem"):
                        kept.append(bytes([prefix]) + encode(feature_length) + feature)
                body = encode(length) + name + encode(len(kept)) + b"".join(kept)
                out += bytes([0]) + encode(len(body)) + body
                at = end
                continue
        out += module[start:end]
        at = end
    return bytes(out)


def members(archive):
    assert archive[:8] == b"!<arch>\n", "not an ar archive"
    at = 8
    names = b""
    while at < len(archive):
        header = archive[at : at + 60]
        name = header[:16].decode().strip()
        size = int(header[48:58])
        body = archive[at + 60 : at + 60 + size]
        at += 60 + size + (size & 1)
        if name == "/":
            continue
        if name == "//":
            names = body
            continue
        if name.startswith("/"):
            offset = int(name[1:])
            name = names[offset : names.index(b"/\n", offset)].decode()
        yield name.rstrip("/"), body


archive, directory = sys.argv[1], sys.argv[2]
os.makedirs(directory, exist_ok=True)
for index, (name, body) in enumerate(members(open(archive, "rb").read())):
    with open(os.path.join(directory, "%04d_%s" % (index, name)), "wb") as out:
        out.write(allow_shared_memory(body))
