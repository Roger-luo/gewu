#!/usr/bin/env python3
"""Offline CLI replay: reconstruct a complete manifest through bounded pages.

No model calls. The input is read only. The output records measured transport
evidence, not a claim about walkthrough quality or provider token counts.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time


def body(patch):
    lines = patch.split("\n")
    if lines[-1] == "":
        lines.pop()
    lines = [line.removesuffix("\r") for line in lines]
    if patch.startswith("@@ "):
        lines = lines[1:]
    return "\n".join(lines)


def check(binary, manifest, max_bytes):
    started = time.monotonic()
    raw = manifest.read_bytes()
    data = json.loads(raw)
    stats = {"calls": 0, "response_bytes": 0, "largest_response_bytes": 0}

    def pages(command):
        cursor = None
        seen = set()
        while True:
            args = [str(binary), "harness", "context", "--input", str(manifest),
                    "--max-bytes", str(max_bytes)]
            if cursor is not None:
                args += ["--cursor", json.dumps(cursor)]
            result = subprocess.run(args + command, capture_output=True, check=True, timeout=30)
            assert len(result.stdout) <= max_bytes, "response exceeded budget"
            page = json.loads(result.stdout)
            assert page["source"] == data["source"], "source identity changed"
            stats["calls"] += 1
            stats["response_bytes"] += len(result.stdout)
            stats["largest_response_bytes"] = max(stats["largest_response_bytes"], len(result.stdout))
            yield page
            cursor = page["next"]
            if cursor is None:
                break
            encoded = json.dumps(cursor, sort_keys=True)
            assert encoded not in seen, "cursor did not advance"
            seen.add(encoded)

    inventory = [entry for page in pages(["inventory"]) for entry in page["entries"]]
    assert [entry["id"] for entry in inventory] == [change["id"] for change in data["changes"]]
    source_pages = 0
    for entry, change in zip(inventory, data["changes"]):
        assert entry["path"] == change["path"]
        assert entry["patch_sha256"] == hashlib.sha256(change["patch"].encode()).hexdigest()
        reconstructed = bytearray()
        for page in pages(["source", "--change-id", change["id"], "--start", "1",
                           "--end", str(change["body_lines"])]):
            assert page["offset"] == len(reconstructed)
            assert page["quote"] == {"change_id": change["id"], "start": 1, "end": change["body_lines"]}
            prefix = reconstructed.decode()
            assert page["first_body_line"] == 1 + prefix.count("\n")
            assert page["first_line_byte"] == len(prefix.rsplit("\n", 1)[-1].encode())
            reconstructed.extend(page["text"].encode())
            source_pages += 1
        assert reconstructed.decode() == body(change["patch"]), change["path"]
    return dict(stats, status="pass", model_calls=0, source=data["source"],
                manifest_sha256=hashlib.sha256(raw).hexdigest(),
                binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                manifest_bytes=len(raw), changes=len(inventory), source_pages=source_pages,
                max_bytes=max_bytes, elapsed_seconds=round(time.monotonic() - started, 3))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--max-bytes", type=int, default=16_384)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    report = check(args.binary.resolve(), args.input.resolve(), args.max_bytes)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
