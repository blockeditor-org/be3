#!/usr/bin/env python3
"""nl-tunnel: carries TCP connections to the build server inside WebSockets, for clients that cannot run WireGuard.

Boat's hosted-port proxy (`host PORT --localhost`) gives this VM an HTTPS address behind a token, and carries
WebSockets but blocks gRPC. A client opens one WebSocket per TCP connection it wants; this accepts the WebSocket on
127.0.0.1, connects to the NativeLink frontend and copies bytes both ways, so buck2's HTTP/2 passes through untouched.
Only the standard library, so the scheduler needs nothing installed.

The port is exposed without boat's own token (`host 8099 --localhost --public`), because boat takes that token only as
a cookie or a query parameter and a Claude Code cloud session's proxy can add only a header. So this authenticates
clients itself: an upgrade must carry `Authorization: Bearer NL_TUNNEL_TOKEN`, and anything else gets 401.

  NL_TUNNEL_PORT    the port to listen on (default 8099), which `host` then exposes
  NL_TUNNEL_BIND    the address to listen on (default 127.0.0.1; a second instance on 10.100.0.1 serves clients inside
                    the WireGuard network, which is how the timings in guides/build_server.md were taken)
  NL_TUNNEL_TARGET  host:port of the frontend (default 10.100.0.1:50052)
  NL_TUNNEL_TOKEN   the token clients must send; required. The nl-scheduler environment delivers it in the secret file
                    ~/nl-tunnel.env, which nl-tunnel.service reads
"""
import asyncio
import base64
import hashlib
import hmac
import os
import struct
import sys

PORT = int(os.environ.get("NL_TUNNEL_PORT", "8099"))
BIND = os.environ.get("NL_TUNNEL_BIND", "127.0.0.1")
HOST, _, TARGET_PORT = os.environ.get("NL_TUNNEL_TARGET", "10.100.0.1:50052").rpartition(":")
TOKEN = os.environ.get("NL_TUNNEL_TOKEN", "").strip()
GUID = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
CHUNK = 65536
PING_EVERY = 20


def log(message):
    print(message, file=sys.stderr, flush=True)


def frame(opcode, payload=b""):
    """A server-to-client frame: not masked."""
    n = len(payload)
    if n < 126:
        head = struct.pack("!BB", 0x80 | opcode, n)
    elif n < 65536:
        head = struct.pack("!BBH", 0x80 | opcode, 126, n)
    else:
        head = struct.pack("!BBQ", 0x80 | opcode, 127, n)
    return head + payload


def unmask(payload, mask):
    if not payload:
        return payload
    repeated = (mask * (len(payload) // 4 + 1))[: len(payload)]
    return (int.from_bytes(payload, "big") ^ int.from_bytes(repeated, "big")).to_bytes(len(payload), "big")


async def read_frame(reader):
    """One frame from the client: (opcode, payload). Client frames are masked."""
    b1, b2 = await reader.readexactly(2)
    opcode = b1 & 0x0F
    length = b2 & 0x7F
    if length == 126:
        (length,) = struct.unpack("!H", await reader.readexactly(2))
    elif length == 127:
        (length,) = struct.unpack("!Q", await reader.readexactly(8))
    mask = await reader.readexactly(4) if b2 & 0x80 else None
    payload = await reader.readexactly(length)
    return opcode, unmask(payload, mask) if mask else payload


async def refuse(writer, status, body):
    writer.write(b"HTTP/1.1 %s\r\nContent-Length: %d\r\nConnection: close\r\n\r\n%s" % (status, len(body), body))
    await writer.drain()


async def handshake(reader, writer):
    """Answers the upgrade request; returns False (after replying) when it is not an authorized WebSocket one."""
    try:
        head = await asyncio.wait_for(reader.readuntil(b"\r\n\r\n"), 15)
    except (asyncio.IncompleteReadError, asyncio.LimitOverrunError, asyncio.TimeoutError):
        return False
    lines = head.decode("latin1").split("\r\n")
    headers = {k.strip().lower(): v.strip() for k, _, v in (line.partition(":") for line in lines[1:]) if k}
    if not hmac.compare_digest(headers.get("authorization", "").encode(), f"Bearer {TOKEN}".encode()):
        await refuse(writer, b"401 Unauthorized", b"nl-tunnel: the token is missing or wrong\n")
        return False
    if headers.get("upgrade", "").lower() != "websocket" or "sec-websocket-key" not in headers:
        await refuse(writer, b"426 Upgrade Required", b"nl-tunnel: a WebSocket upgrade is what this serves\n")
        return False
    accept = base64.b64encode(hashlib.sha1(headers["sec-websocket-key"].encode() + GUID).digest()).decode()
    writer.write(
        ("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
         f"Sec-WebSocket-Accept: {accept}\r\n\r\n").encode()
    )
    await writer.drain()
    return True


async def client(reader, writer):
    try:
        if not await handshake(reader, writer):
            return
        try:
            target_reader, target_writer = await asyncio.open_connection(HOST, int(TARGET_PORT))
        except OSError as error:
            log(f"cannot reach {HOST}:{TARGET_PORT}: {error}")
            writer.write(frame(0x8, struct.pack("!H", 1011) + b"build server unreachable"))
            await writer.drain()
            return

        async def to_target():
            while True:
                opcode, payload = await read_frame(reader)
                if opcode in (0x0, 0x1, 0x2):          # data, whichever way the client framed it
                    target_writer.write(payload)
                    await target_writer.drain()
                elif opcode == 0x9:                    # ping
                    writer.write(frame(0xA, payload))
                elif opcode == 0x8:                    # close
                    writer.write(frame(0x8, payload[:2]))
                    return

        async def to_client():
            while True:
                data = await target_reader.read(CHUNK)
                if not data:
                    writer.write(frame(0x8, struct.pack("!H", 1000)))
                    return
                writer.write(frame(0x2, data))
                await writer.drain()

        async def pinger():
            while True:
                await asyncio.sleep(PING_EVERY)
                writer.write(frame(0x9))

        tasks = [asyncio.ensure_future(job()) for job in (to_target, to_client, pinger)]
        try:
            await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
        finally:
            for task in tasks:
                task.cancel()
            target_writer.close()
    except (asyncio.IncompleteReadError, ConnectionError, OSError):
        pass
    finally:
        writer.close()


async def main():
    if not TOKEN:
        sys.exit("nl-tunnel: NL_TUNNEL_TOKEN is not set (the nl-scheduler environment's secret file ~/nl-tunnel.env)")
    server = await asyncio.start_server(client, BIND, PORT, limit=1 << 20)
    log(f"nl-tunnel: {BIND}:{PORT} -> {HOST}:{TARGET_PORT}")
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    asyncio.run(main())
