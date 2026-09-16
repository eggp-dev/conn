#!/usr/bin/env python3
"""Run explicitly queued Conn agent RPCs over one persistent local connection.

Append one JSON object per line to --queue: {"method": "snapshot", "params": {}}.
The driver reads an existing queue from its beginning; use a fresh file per take.
It never grants approvals, sends human input, or invents commands. Stopping it
disconnects the agent; enqueue release_control explicitly when appropriate.
"""

import argparse
import json
import select
import signal
import socket
import sys
import time
from pathlib import Path


METHODS = {
    "snapshot", "status", "affordances", "request_control", "release_control",
    "type", "send_key", "check_approval", "control_request_state", "exec_state",
    "proposal_state", "analyse", "interrupt", "list_tabs", "open_tab",
    "switch_tab", "request_attention",
}


def redact(value):
    if isinstance(value, dict):
        return {key: "[redacted]" if "token" in key.lower() else redact(item)
                for key, item in value.items()}
    if isinstance(value, list):
        return [redact(item) for item in value]
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", required=True, type=Path)
    parser.add_argument("--queue", required=True, type=Path)
    parser.add_argument("--agent-id", default="codex")
    parser.add_argument("--log", type=Path)
    parser.add_argument("--timeout", type=float, default=600,
                        help="Maximum seconds for each RPC, including UI approval")
    args = parser.parse_args()
    if args.timeout <= 0:
        parser.error("--timeout must be positive")

    log = args.log.open("a", encoding="utf-8") if args.log else None
    stopped = False
    connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    incoming = bytearray()
    next_id = 0

    def emit(value):
        encoded = json.dumps(redact(value), ensure_ascii=False)
        print(encoded, flush=True)
        if log:
            log.write(encoded + "\n")
            log.flush()

    def stop(_signum, _frame):
        nonlocal stopped
        stopped = True
        try:
            connection.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)

    def receive(wait):
        messages = []
        if b"\n" not in incoming:
            ready, _, _ = select.select([connection], [], [], max(0, wait))
            if not ready:
                return messages
            data = connection.recv(65536)
            if not data:
                raise ConnectionError("Conn agent socket closed")
            incoming.extend(data)
        while b"\n" in incoming:
            line, _, remainder = incoming.partition(b"\n")
            incoming[:] = remainder
            if line.strip():
                messages.append(json.loads(line))
        return messages

    def call(method, params, label=None):
        nonlocal next_id
        next_id += 1
        request_id = next_id
        payload = {"id": request_id, "method": method, "params": params}
        connection.sendall((json.dumps(payload) + "\n").encode())
        deadline = time.monotonic() + args.timeout
        while not stopped:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"RPC {request_id} ({method}) timed out; stopping to avoid uncertain replay")
            result = None
            for message in receive(min(remaining, 0.5)):
                record = {"receivedAt": time.time(), "message": message}
                if message.get("id") == request_id:
                    record.update({"method": method, "label": label})
                    result = message
                emit(record)
            if result is not None:
                return result
        return None

    try:
        connection.settimeout(args.timeout)
        connection.connect(str(args.socket))
        connection.settimeout(None)
        hello = call("hello", {"kind": "agent", "agentId": args.agent_id,
                               "name": args.agent_id}, "connect")
        if hello is None or "error" in hello:
            raise ConnectionError("Conn did not accept agent identity")
        args.queue.touch(exist_ok=True)
        with args.queue.open(encoding="utf-8") as queue:
            pending = ""
            while not stopped:
                pending += queue.read()
                if "\n" in pending:
                    line, pending = pending.split("\n", 1)
                    if not line.strip():
                        continue
                    try:
                        job = json.loads(line)
                        if not isinstance(job, dict):
                            raise ValueError("Expected a JSON object")
                        method = job["method"]
                        params = job.get("params", {})
                        if method not in METHODS or not isinstance(params, dict):
                            raise ValueError("Expected an agent RPC method and object params")
                    except (ValueError, KeyError, TypeError) as error:
                        emit({"queueError": str(error)})
                        continue
                    call(method, params, job.get("label"))
                else:
                    for message in receive(0.2):
                        emit({"receivedAt": time.time(), "message": message})
    except (OSError, ValueError, TimeoutError) as error:
        if not stopped:
            emit({"driverError": str(error)})
            return 1
    finally:
        connection.close()
        if log:
            log.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
