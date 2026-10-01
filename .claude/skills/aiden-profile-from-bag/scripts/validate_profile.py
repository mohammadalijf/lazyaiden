#!/usr/bin/env python3
"""Validate a Fellow Aiden profile YAML with the same rules as the lazyaiden repo
(crates/fellow-client/src/validation.rs), plus a temperature-list length check.

Usage: python3 validate_profile.py profile.yaml [more.yaml ...]
Exit code 0 when every file is valid, 1 otherwise. No third-party dependencies.
"""

import re
import sys

KEYS = [
    "profileType", "title", "ratio", "bloomEnabled", "bloomRatio", "bloomDuration",
    "bloomTemperature", "ssPulsesEnabled", "ssPulsesNumber", "ssPulsesInterval",
    "ssPulseTemperatures", "batchPulsesEnabled", "batchPulsesNumber",
    "batchPulsesInterval", "batchPulseTemperatures",
]
TITLE_PUNCTUATION = "!@#$%&*-+?/.,:)("


def scalar(text):
    text = text.strip()
    if len(text) >= 2 and text[0] == text[-1] and text[0] in "'\"":
        return text[1:-1]
    low = text.lower()
    if low in ("true", "false"):
        return low == "true"
    try:
        return int(text)
    except ValueError:
        pass
    try:
        return float(text)
    except ValueError:
        return text


def strip_comment(line):
    out, quote = [], None
    for i, c in enumerate(line):
        if quote:
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c == "#" and (i == 0 or line[i - 1].isspace()):
            break
        out.append(c)
    return "".join(out).rstrip()


def parse(text):
    """Parse the flat profile format: `key: value`, inline `[a, b]` lists and `- x` block lists."""
    data, current = {}, None
    for raw in text.splitlines():
        line = strip_comment(raw)
        if not line.strip() or line.strip() == "---":
            continue
        stripped = line.strip()
        if stripped.startswith("- "):
            if current is None or not isinstance(data.get(current), list):
                raise ValueError(f"list item without a list key: {raw!r}")
            data[current].append(scalar(stripped[2:]))
            continue
        m = re.match(r"^([A-Za-z_][\w]*)\s*:\s*(.*)$", line)
        if not m:
            raise ValueError(f"cannot parse line: {raw!r}")
        key, value = m.group(1), m.group(2).strip()
        current = key
        if value == "":
            data[key] = []
        elif value.startswith("[") and value.endswith("]"):
            inner = value[1:-1].strip()
            data[key] = [scalar(v) for v in inner.split(",")] if inner else []
        else:
            data[key] = scalar(value)
    return data


def on_grid(v, lo, hi, step):
    if isinstance(v, bool) or not isinstance(v, (int, float)):
        return False
    if v < lo - 1e-9 or v > hi + 1e-9:
        return False
    k = (v - lo) / step
    return abs(k - round(k)) < 1e-9


def issues(p):
    out = []
    missing = [k for k in KEYS if k not in p]
    if missing:
        out.append(f"missing keys: {', '.join(missing)}")
    extra = [k for k in p if k not in KEYS]
    if extra:
        out.append(f"warning (ignored on import): unknown keys {', '.join(extra)}")

    t = p.get("title")
    if not isinstance(t, str) or not t:
        out.append("title: must be a non-empty string")
    else:
        if len(t) > 50:
            out.append("title: must be at most 50 characters")
        bad = [c for c in t if not ((c.isascii() and c.isalnum()) or c == " " or c in TITLE_PUNCTUATION)]
        if bad:
            out.append(f"title: unsupported characters {''.join(sorted(set(bad)))!r}")

    if "profileType" in p and p["profileType"] != 0:
        out.append("profileType: should be 0")
    for k in ("bloomEnabled", "ssPulsesEnabled", "batchPulsesEnabled"):
        if k in p and not isinstance(p[k], bool):
            out.append(f"{k}: must be true or false")
    if "ratio" in p and not on_grid(p["ratio"], 14, 20, 0.5):
        out.append(f"ratio: {p['ratio']} is not one of 14-20 in steps of 0.5")
    if "bloomRatio" in p and not on_grid(p["bloomRatio"], 1, 3, 0.5):
        out.append(f"bloomRatio: {p['bloomRatio']} is not one of 1-3 in steps of 0.5")
    for k, lo, hi in (("bloomDuration", 1, 120), ("ssPulsesNumber", 1, 10), ("ssPulsesInterval", 5, 60),
                      ("batchPulsesNumber", 1, 10), ("batchPulsesInterval", 5, 60)):
        if k in p and not (isinstance(p[k], int) and not isinstance(p[k], bool) and lo <= p[k] <= hi):
            out.append(f"{k}: {p[k]} must be a whole number {lo}-{hi}")
    if "bloomTemperature" in p and not on_grid(p["bloomTemperature"], 50, 98.5, 0.5):
        out.append(f"bloomTemperature: {p['bloomTemperature']} is not one of 50-98.5 in steps of 0.5")
    for prefix in ("ss", "batch"):
        key = f"{prefix}PulseTemperatures"
        temps = p.get(key)
        if key not in p:
            continue
        if not isinstance(temps, list) or not temps:
            out.append(f"{key}: must be a non-empty list")
            continue
        for v in temps:
            if not on_grid(v, 50, 98.5, 0.5):
                out.append(f"{key}: {v} is not one of 50-98.5 in steps of 0.5")
        n = p.get(f"{prefix}PulsesNumber")
        if isinstance(n, int) and len(temps) != n:
            out.append(f"{key}: has {len(temps)} entries but {prefix}PulsesNumber is {n}")
    return out


def main(paths):
    if not paths:
        print(__doc__.strip())
        return 2
    ok = True
    for path in paths:
        try:
            with open(path, encoding="utf-8") as f:
                problems = issues(parse(f.read()))
        except (OSError, ValueError) as e:
            problems = [str(e)]
        errors = [x for x in problems if not x.startswith("warning")]
        ok = ok and not errors
        print(f"{'OK  ' if not errors else 'FAIL'} {path}")
        for x in problems:
            print(f"     {x}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
