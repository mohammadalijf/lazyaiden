---
name: aiden-profile-from-bag
description: Turn a coffee bag (photos of the bag, or a link to the roaster's product page) into a validated Fellow Aiden brew-profile YAML that can be imported with the lazyaiden TUI or lazyaiden-cli. Use when the user shares a coffee bag image or URL, says "make an Aiden profile for this coffee", "new bag", "dial in this coffee", or asks to tune an existing Aiden profile from taste feedback (sour, bitter, flat...).
---

# Aiden profile from a coffee bag

Produces a Fellow Aiden profile YAML tuned to the coffee on the bag, from a one-time research base so the answer is
consistent every time. **Do not redo web research**; everything needed is bundled:

* `references/field-research.md`: what every field does to taste, allowed ranges, bag-to-setting rules (§7), anchor
  profiles (§8), the user's own style (§8b), the taste-to-fix table (§9), evidence levels (§10). **Read it fully first.**
* `references/user-profiles/*.yaml`: the user's real, tasted profiles. They calibrate taste; prefer their style over
  generic defaults when the coffee is similar.
* `scripts/validate_profile.py`: dependency-free validator (same rules as the lazyaiden app, plus a pulse-count check).

## Profile format (all 15 keys required, camelCase, temperatures in °C)

```yaml
profileType: 0
title: Roaster Coffee Process     # 1-50 chars: ASCII letters, digits, space, ! @ # $ % & * - + ? / . , : ) (
ratio: 16.5                       # 1:x, 14-20 in 0.5 steps
bloomEnabled: true
bloomRatio: 3.0                   # 1-3 in 0.5 steps (water = dose x bloomRatio)
bloomDuration: 45                 # seconds, integer 1-120
bloomTemperature: 96.0            # 50-98.5 in 0.5 steps
ssPulsesEnabled: true             # single serve (up to ~450 ml)
ssPulsesNumber: 3                 # 1-10
ssPulsesInterval: 20              # seconds, 5-60
ssPulseTemperatures: [95.0, 93.0, 91.0]    # exactly ssPulsesNumber entries
batchPulsesEnabled: true          # batch (451-1500 ml)
batchPulsesNumber: 3
batchPulsesInterval: 30
batchPulseTemperatures: [95.0, 93.0, 91.0] # exactly batchPulsesNumber entries
```

## Workflow

1. **Collect the bag info.**
   * Images: look at every image (front, back, side label). Link: fetch the page and pull out roaster, coffee name,
     origin/region/farm/washing station, variety, **process**, **altitude**, **roast level**, **roast date**, tasting
     notes, the roaster's own brew recommendation, bag weight.
   * Record what you found **and what is missing**. Never invent a value. If the roast level is not stated, infer it
     from the description and tasting notes and say that you did.
   * If a roaster recipe is in °F, convert with the table in research §8.
2. **Ask only what you can't infer, in one short message**, and skip anything already known:
   * roast date, if not on the bag (drives bloom time);
   * grinder (Ode Gen 2 / Ode + SSP / Opus / other), for the grind note;
   * cup preference if the bag gives no clue (default: the user's style in §8b, which leans toward clarity);
   * their usual dose only if they want it in the title (their pattern: `<Name> <dose>grams <Coffee>`).
   If the user says "just do it", use defaults and list them as assumptions.
3. **Build the profile.** Start from the user's profile or the nearest anchor (§8), then move each field per §3-§7:
   roast level first, then process, altitude/variety, freshness, tasting notes. Then check:
   * ratio, bloom ratio and every temperature are on their 0.5 grids and in range; durations are integers;
   * each temperature list has exactly `…PulsesNumber` entries;
   * batch interval ≈ single-serve interval + 10 s; both sets follow the same temperature-curve shape;
   * when a pulse set is disabled, keep `Number: 1`, a one-entry temperature list and a valid interval;
   * title is ≤ 50 chars, accents transliterated (é → e, ñ → n), no apostrophes.
4. **Write the file** as `<slug>.yaml` (slug = lowercase title, spaces → hyphens).
   * Claude Code: save to `./lazyaiden-profiles/<slug>.yaml` in the working directory unless the user names a place. Never
     overwrite; add `-2`, `-3`.
   * claude.ai: create the file as a downloadable output.
5. **Validate** and fix until it passes:
   ```bash
   python3 <skill-dir>/scripts/validate_profile.py <slug>.yaml
   ```
   Inside the lazyaiden repo you can also double-check with the app's own validator, using a throwaway store:
   `cargo run -q -p lazyaiden-cli -- --profiles-dir "$(mktemp -d)" profile add -i <slug>.yaml`.
6. **Present**, concisely, in this order:
   1. The YAML in a code block (and the file).
   2. A short table of what you read from the bag vs what you assumed.
   3. "Why these settings": 4-6 bullets that tie bag facts to fields (e.g. "washed, 2,100 masl, light → 96 °C bloom,
      3 pulses falling 2 °C each, ratio 16.5 for clarity").
   4. **Grind note**: direction plus a number when there is an anchor (§7). The YAML has no grind field. Say "start here and taste".
   5. How to import (below), and the first-cup check: ask for one word (sour / bitter / flat / hollow / muddy / good) to
      tune with §9, **one lever at a time**.
   State the limits honestly: the bag can't reveal water, grinder or roast development, and the per-process offsets are heuristics.

## Importing the result (tell the user, don't run unless asked)

```bash
lazyaiden-cli profile add -i lazyaiden-profiles/<slug>.yaml   # add to the local profile store
lazyaiden-cli profile push <slug>                         # send it to the brewer (appears in the Fellow app too)
```
In the `lazyaiden` TUI: press `i` and give the file path, then `p` to push.

## Tuning an existing profile

Given a profile (file or pasted YAML) plus taste feedback: don't rebuild it. Apply §9, change one lever, show the exact
field and old → new value, validate, and save it as `<slug>-v2.yaml` unless told to overwrite. If the user says the cup
was great, offer to copy the profile into `references/user-profiles/` so future profiles learn their taste.

## Guardrails

* Never push to the brewer, create schedules or log in unless explicitly asked. Never read or print credentials.
* If an image is unreadable or a link is blocked, say so and ask for the missing facts instead of guessing.
* Research is dated 2026-10-01. To refresh it, edit `references/field-research.md` instead of answering from memory.
