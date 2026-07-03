#!/usr/bin/env python3
"""Deterministic generator for the boundary-benchmark workloads.

The three workloads are the ones defined byte-exactly in the appendix of
tools/benchmark/BINDINGS-OVERHEAD.md:

  | name        | rule    | data    |
  |-------------|---------|---------|
  | simple      |    74 B |    68 B |
  | eligibility |   458 B |   955 B |
  | array100    |    89 B | 8,279 B |

Everything is single-line JSON in Python's default `json.dumps` style
(", " item separator, ": " key separator) — that is the formatting that
reproduces the documented byte counts. The generated files are CHECKED IN
so runs stay byte-stable across machines; this script exists so the
provenance of every byte is auditable and regenerable.

Usage:
  python3 generate.py          # (re)write the workload + expected files
  python3 generate.py --check  # verify the checked-in files match, exit 1 on drift

Expected results (verified by every runner before timing starts):
  simple      -> true
  eligibility -> "approved:APP-2481"
  array100    -> the qty array of the 49 items with price > 250

Expected files hold the result serialized the way the engine emits JSON
strings: compact (no spaces), which is what the string-out API tiers
return byte-for-byte.
"""

import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))

# --------------------------------------------------------------------------
# simple — rule 74 B, data 68 B (verbatim from the appendix)
# --------------------------------------------------------------------------

SIMPLE_RULE = (
    '{"and": [{">": [{"var": "age"}, 18]}, '
    '{"==": [{"var": "country"}, "US"]}]}'
)
SIMPLE_DATA = '{"age": 21, "country": "US", "name": "Ada Lovelace", "tier": "gold"}'
SIMPLE_EXPECTED = "true"

# --------------------------------------------------------------------------
# eligibility — rule 458 B (appendix rule, single-line normalized),
# data 955 B (concrete payload constructed to the appendix's structural
# sketch: applicant{id,age:34,income:52000,credit_score:715,
# debt_ratio:0.22,ssn,state:"CA",address{street,city,zip:"95014",country},
# employment{...},accounts[3],flags{...}}, meta{...}, padding[8 x 32-char])
# --------------------------------------------------------------------------

ELIGIBILITY_RULE_OBJ = {
    "if": [
        {
            "and": [
                {">=": [{"var": "applicant.age"}, 21]},
                {"<": [{"var": "applicant.age"}, 65]},
                {
                    "or": [
                        {">=": [{"var": "applicant.income"}, 45000]},
                        {
                            "and": [
                                {">=": [{"var": "applicant.credit_score"}, 700]},
                                {"<=": [{"var": "applicant.debt_ratio"}, 0.3]},
                            ]
                        },
                    ]
                },
                {"!": {"missing": ["applicant.ssn", "applicant.address.zip"]}},
                {"in": [{"var": "applicant.state"}, ["CA", "NY", "TX", "WA", "MA"]]},
            ]
        },
        {"cat": ["approved:", {"var": "applicant.id"}]},
        "rejected",
    ]
}

# 8 deterministic 32-char padding strings: rotations of a fixed alphabet.
_PAD_BASE = "0123456789abcdefghijklmnopqrstuv"
ELIGIBILITY_DATA_OBJ = {
    "applicant": {
        "id": "APP-2481",
        "age": 34,
        "income": 52000,
        "credit_score": 715,
        "debt_ratio": 0.22,
        "ssn": "543-21-6789",
        "state": "CA",
        "tier": "gold-member",
        "address": {
            "street": "1912 Ada Ct",
            "city": "Cupertino",
            "zip": "95014",
            "country": "US",
        },
        "employment": {
            "employer": "Plasmatic Systems",
            "role": "engineer",
            "years": 6,
            "status": "full_time",
        },
        "accounts": [
            {"type": "checking", "balance": 8421},
            {"type": "savings", "balance": 26350},
            {"type": "credit", "balance": -1240},
        ],
        "flags": {"kyc_verified": True, "fraud_hold": False, "prior_default": False},
    },
    "meta": {
        "request_id": "req-7f3a2c91",
        "channel": "api",
        "ts": "2026-07-03T09:15:00Z",
    },
    "padding": [_PAD_BASE[i:] + _PAD_BASE[:i] for i in range(8)],
}
ELIGIBILITY_EXPECTED = '"approved:APP-2481"'

# --------------------------------------------------------------------------
# array100 — rule 89 B, data 8,279 B (appendix generator formula:
# price=(i*37)%500, qty=i%7, name="item-%04d", tags=["retail","q3"],
# for i in 0..100)
# --------------------------------------------------------------------------

ARRAY100_RULE = (
    '{"map": [{"filter": [{"var": "items"}, '
    '{">": [{"var": "price"}, 250]}]}, {"var": "qty"}]}'
)


def array100_items():
    return [
        {
            "id": i,
            "price": (i * 37) % 500,
            "qty": i % 7,
            "name": "item-%04d" % i,
            "tags": ["retail", "q3"],
        }
        for i in range(100)
    ]


def build_files():
    """Return {filename: exact_bytes} for every workload artifact."""
    items = array100_items()
    array100_data = json.dumps({"items": items})
    # Engine string-out APIs serialize compactly (no spaces).
    array100_expected = json.dumps(
        [it["qty"] for it in items if it["price"] > 250], separators=(",", ":")
    )
    files = {
        "simple.rule.json": SIMPLE_RULE,
        "simple.data.json": SIMPLE_DATA,
        "simple.expected.json": SIMPLE_EXPECTED,
        "eligibility.rule.json": json.dumps(ELIGIBILITY_RULE_OBJ),
        "eligibility.data.json": json.dumps(ELIGIBILITY_DATA_OBJ),
        "eligibility.expected.json": ELIGIBILITY_EXPECTED,
        "array100.rule.json": ARRAY100_RULE,
        "array100.data.json": array100_data,
        "array100.expected.json": array100_expected,
    }
    return {name: text.encode("utf-8") for name, text in files.items()}


# Documented sizes from BINDINGS-OVERHEAD.md; the generator refuses to
# emit anything that drifts from them.
DOCUMENTED_SIZES = {
    "simple.rule.json": 74,
    "simple.data.json": 68,
    "eligibility.rule.json": 458,
    "eligibility.data.json": 955,
    "array100.rule.json": 89,
    "array100.data.json": 8279,
}


def main() -> int:
    check = "--check" in sys.argv[1:]
    files = build_files()

    for name, size in DOCUMENTED_SIZES.items():
        actual = len(files[name])
        if actual != size:
            print(
                f"generate.py: {name} is {actual} B, documented size is {size} B",
                file=sys.stderr,
            )
            return 1

    drift = []
    for name, payload in files.items():
        path = os.path.join(HERE, name)
        if check:
            try:
                with open(path, "rb") as f:
                    on_disk = f.read()
            except FileNotFoundError:
                on_disk = None
            if on_disk != payload:
                drift.append(name)
        else:
            with open(path, "wb") as f:
                f.write(payload)

    if check:
        if drift:
            print(
                "generate.py --check: checked-in workloads drifted from the "
                "generator: " + ", ".join(drift),
                file=sys.stderr,
            )
            return 1
        print("generate.py --check: all workload files match the generator")
        return 0

    for name, payload in sorted(files.items()):
        print(f"wrote {name} ({len(payload)} B)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
