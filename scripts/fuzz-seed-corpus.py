#!/usr/bin/env python3
"""Write a libFuzzer seed corpus for the `eval_str` fuzz target from the
JSONLogic conformance suites.

The target takes `(bool, &str, &str)` = (templating, rule, data), decoded
by `arbitrary` 1.x from the raw input: the bool from the first byte (bit
0); the rule's length from the *end* of the remaining bytes (one byte up
to 256 remaining, then two bytes, big-endian, up to 65,537), then the rule
from the front; the data is whatever is left. Each suite case is encoded
that way, so the fuzzer starts from every rule the suites exercise rather
than from nothing. A case too large for the two-byte form is skipped.

Usage: scripts/fuzz-seed-corpus.py OUT_DIR
"""
import hashlib
import json
import os
import sys

SUITES = os.path.join(os.path.dirname(__file__), "..", "crates", "datalogic-rs", "tests", "suites")


def encode(templating, rule, data):
    rule_b = rule.encode("utf-8")
    data_b = data.encode("utf-8")
    body = rule_b + data_b
    n = len(rule_b)
    # Length of what follows the flag byte, including the length suffix.
    if len(body) + 1 <= 256:
        suffix = bytes([n])
    elif len(body) + 2 <= 65537:
        max_size = len(body)  # = remaining - 2
        # `arbitrary` reads only as many suffix bytes as the range needs:
        # one while max_size < 256.
        suffix = bytes([n, 0]) if max_size < 256 else bytes([n >> 8, n & 0xFF])
    else:
        return None
    return bytes([1 if templating else 0]) + body + suffix


def decode(blob):
    """The `arbitrary` 1.x decoding, to self-check `encode`."""
    templating = blob[0] & 1 == 1
    rest = blob[1:]
    if len(rest) <= 1:
        return templating, "", rest.decode("utf-8", "replace")
    if len(rest) <= 256:
        max_size = len(rest) - 1
        size = rest[-1] % (max_size + 1)
        rest = rest[:-1]
    else:
        max_size = len(rest) - 2
        suffix = rest[-2:]
        rest = rest[:-2]
        v = suffix[0] if max_size < 256 else (suffix[0] << 8) | suffix[1]
        size = v % (max_size + 1)
    return templating, rest[:size].decode("utf-8"), rest[size:].decode("utf-8")


def main(argv):
    if len(argv) != 1:
        print(__doc__, file=sys.stderr)
        return 2
    out = argv[0]
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(SUITES, "index.json"), encoding="utf-8") as f:
        index = json.load(f)
    written = skipped = 0
    for name in index:
        with open(os.path.join(SUITES, name), encoding="utf-8") as f:
            entries = json.load(f)
        for case in entries:
            if not isinstance(case, dict) or "rule" not in case:
                continue
            templating = case.get("templating") is True
            rule = json.dumps(case["rule"], separators=(",", ":"), ensure_ascii=False)
            data = json.dumps(case.get("data", None), separators=(",", ":"), ensure_ascii=False)
            blob = encode(templating, rule, data)
            if blob is None:
                skipped += 1
                continue
            assert decode(blob) == (templating, rule, data), (name, rule)
            path = os.path.join(out, hashlib.sha1(blob).hexdigest())
            with open(path, "wb") as f:
                f.write(blob)
            written += 1
    print(f"fuzz-seed-corpus: wrote {written} seeds to {out} ({skipped} too large)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
