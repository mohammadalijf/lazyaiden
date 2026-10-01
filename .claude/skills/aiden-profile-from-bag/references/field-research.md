# Aiden profile fields: what each one does to the cup

One-time research, reused by the `aiden-profile-from-bag` skill. Compiled 2026-10-01.

**How to read the confidence tags**

* **[S] Science**: peer-reviewed or measured. Trust it.
* **[F] Fellow**: stated by Fellow (manual, help centre, official recipes). Trust it for how the machine behaves.
* **[E] Expert/community consensus**: roasters, authors (e.g. Scott Rao), forum users. Reasonable defaults, not proven.
* **[H] Heuristic**: my synthesis from the above. Treat as a starting point to be corrected by tasting.

Things I could not verify (Fellow help-centre pages and two Wiley papers returned 403) are marked *unverified*.

---

## 0. The three ideas that explain everything

1. **Taste is driven by strength (TDS) and extraction yield (PE), not by any single knob.** A UC Davis panel
   (12 trained tasters, 270 brews, 31 attributes) held TDS and PE constant while changing water temperature
   (87/90/93 °C) and found **no appreciable sensory difference** from temperature. What did matter [S]:
   * **Higher TDS** (stronger): more bitter, astringent, smoky; lower TDS: more fruity, floral.
   * **Higher PE** (more extracted): more brown-roast and ashy; lower PE: more sour, citrus, berry.
   
   Source: Batali et al., *Brew temperature, at fixed brew strength and extraction, has little impact on the sensory
   profile of drip brew coffee*, https://pmc.ncbi.nlm.nih.gov/articles/PMC7536440/
2. **Every YAML field is a lever on PE or TDS.** Temperature, bloom, pulse count and interval move **extraction yield**;
   `ratio` moves **strength**. Temperature is not magic flavour: it is the lever that is cheapest to turn on a brewer
   that does not let you change grind or time directly. (Temperature still changes *how fast* things dissolve, and the
   Aiden's per-pulse temperatures let you shape extraction over time.)
3. **The YAML does not contain the grind.** Grind is the other big extraction lever and lives on the grinder. A profile
   can only be as good as the grind it is paired with, so the skill always states a grind direction (finer / coarser)
   alongside the YAML.

Fellow's own advice [F]: temperature and ratio "have the largest impact on flavor"; "the higher the temperature and the
more pulses you have, the higher the extraction". If it tastes bitter, shorten the bloom; if sour or flat, lengthen it.

Roast/temperature chemistry [S]: roast level changes acidity (light pH ~4.8 hot, dark ~5.4), chlorogenic acid (light
~1,535 mg/L vs dark ~1,308 mg/L) and TDS (dark extracts more); caffeine is essentially unaffected by temperature or roast
at these conditions (~1,000-1,100 mg/L). Cold water extracts less melanoidin, which is why cold brew is less bitter/acidic.
Source: https://pmc.ncbi.nlm.nih.gov/articles/PMC7404565/

Consumer reality [S, from the paper titles/abstracts; full text 403]: preferences for brew strength and extraction are spread over a wide
range (Cotter et al. 2021; Guinard et al. 2023 brewing control chart). The classic "18-22 % extraction, 1.15-1.35 % TDS"
box is a guide, not a law. **Taste feedback beats the rules** (see section 9).

---

## 1. Field map

| YAML field | Range (validated) | Primary lever on | Section |
| --- | --- | --- | --- |
| `profileType` | `0` | none (always 0) | 2 |
| `title` | 1-50 chars, ASCII | none | 2 |
| `ratio` | 14-20, step 0.5 | **strength (TDS)**, body | 3 |
| `bloomEnabled` | bool | evenness at the start | 4 |
| `bloomRatio` | 1-3, step 0.5 | wetting completeness | 4 |
| `bloomDuration` | 1-120 s | degassing, early extraction | 4 |
| `bloomTemperature` | 50-98.5 °C, step 0.5 | early extraction speed | 4 |
| `ssPulsesEnabled` | bool | agitation mode (single serve) | 5 |
| `ssPulsesNumber` | 1-10 | **extraction yield**, agitation | 5 |
| `ssPulsesInterval` | 5-60 s | contact time, drawdown between pours | 5 |
| `ssPulseTemperatures` | list, 50-98.5 step 0.5 | extraction curve over time | 6 |
| `batchPulsesEnabled` | bool | same, batch mode | 5 |
| `batchPulsesNumber` | 1-10 | same | 5 |
| `batchPulsesInterval` | 5-60 s | same | 5 |
| `batchPulseTemperatures` | list, 50-98.5 step 0.5 | same | 6 |

How the machine uses them [F]: bloom volume = coffee grams × `bloomRatio` (20 g × 3 = 60 ml). The remaining water is
**split equally across the pulses**, so more pulses means smaller pours. Small brews use the single-serve pulse set
and larger ones the batch set; the same profile carries both. Volumes per Fellow's guide (via Seattle Coffee Gear): single
serve up to 450 ml (green basket, #2 cone filter), batch 451-1500 ml (blue basket, flat filter); some secondary sources say
the switch is ~500 ml. Dose limits: 82.5 g for batch, 110 g for cold brew.

---

## 2. `profileType`, `title`

* `profileType`: always `0` (regular profile). Do not invent other values.
* `title`: shown on the brewer. Keep it ≤ 50 chars, ASCII letters/digits/space and `! @ # $ % & * - + ? / . , : ) (`.
  No accents, apostrophes, hyphens-in-quotes or emoji; transliterate (`Ethiopia Yirgacheffe Konga` yes, `Café Ñandú` no).
  Good pattern: `<Roaster> <Coffee> <Process>` e.g. `Lineage Gicherori AA Washed`. Putting the roast date in the
  title is optional but useful when rolling through bags.

---

## 3. `ratio` (1:x), strength

**What it does.** Grams of water per gram of coffee. At the same extraction yield, a lower number (1:15) makes a
stronger, heavier brew; a higher number (1:17-18) makes a lighter, cleaner one. [S] via the UC Davis result: TDS drives
bitter/astringent/smoky up and fruity/floral down.

| Ratio | Cup character | Typical use |
| --- | --- | --- |
| 14-15 | Intense, heavy body, dense, risk of smoky/harsh | Dark roasts for milk; people who like strong |
| 15.5-16 | Balanced, full, sweet | Default; medium roasts; large batches |
| 16.5-17 | Light, clean, more clarity, fruit/floral lift | Light roasts, delicate washed coffees |
| 17.5-20 | Tea-like, delicate, easy to under-extract/watery | Very high-end, very light, or very sour-prone: rarely |

* Fellow presets [F via secondary sources]: light 1:17, medium 1:16, dark 1:16.
* Some pour-over guides suggest the opposite (stronger 1:15 for light, 1:17-18 for dark) [E, weak, unverified
  attribution]. Aiden recipes from Fellow, roasters and the community put light roasts at 1:16-17. Reconciliation
  [H]: the brewer's heat and pulses already extract light roasts hard, and a higher ratio keeps clarity; dark roasts
  are strong-tasting at any ratio, so diluting them is also reasonable. **Use the Aiden convention, but let the user's
  strength preference override** ("I want it bolder" → drop 0.5-1.0).
* Ratio changes strength and, indirectly, extraction (more water per gram tends to extract more of the coffee).
  Adjust ratio for **strength**; adjust temperature/pulses/grind for **extraction**.
* Because `ratio` also sets total water, small ratio steps (0.5) are about a 3 % strength change. Move in 0.5s.

---

## 4. Bloom: `bloomEnabled`, `bloomRatio`, `bloomDuration`, `bloomTemperature`

### What the bloom is for [E, mechanism is well established; magnitudes are not]
Freshly roasted coffee holds CO₂. Gas escaping from the grounds repels water, so unblooming coffee wets unevenly and
channels. The bloom pre-wets everything, lets most of the gas leave, and then the main pours extract evenly. Many web
pages quote precise gains from blooming ("22 % more consistent"); I could not trace those to a real study, so ignore
the numbers. The direction (more even) is consistent with Fellow's guidance.

### `bloomEnabled`
* `true` for essentially everything. Only consider `false` for old, fully degassed coffee in a rush; the cost is small
  either way. If `false`, still keep valid values in the other bloom fields (the validator requires them).

### `bloomRatio` (water : coffee, 1-3)
* Fellow default 2 (1:2); Rao recommends ~3 for full saturation [E]. Aiden grid: 1, 1.5, 2, 2.5, 3.
* **Higher (2.5-3)**: guarantees every particle is wet, more early extraction of the fast-dissolving acids and
  sugars, better for dense/light/very fresh coffee and for fine grinds that clump.
* **Lower (1-2)**: less early extraction, gentler; fine for dark, very porous, stale or coarse-ground coffee where
  you worry about dumping bitterness early.
* Ratio < 2 can leave dry pockets in a deep bed (large batches). In batch mode keep ≥ 2.
* Evidence level: [E]. Fellow presets use 3 for light and 2 for medium/dark.

### `bloomDuration` (seconds, 1-120)
* Fellow default 30 s; typical 30-45 s; Fellow's tuning rule [F]: *bitter → shorten, sour/flat → lengthen.*
* **Longer** gives more complete degassing and more early extraction. Use for: very fresh coffee (< 7 days off roast),
  dense light roasts, dark oily roasts that degas hard (45-60 s) [E].
* **Shorter (20-30 s)** for stale (> 4 weeks), already-degassed, or bitter-leaning coffees.
* Degassing facts [F, E]: ~40 % of CO₂ leaves in the first 24 h; peak drip window is ~4-14 days after roast; darker roasts
  degas faster (dark may lose 60-70 % of CO₂ in 3 days, light spreads it over 7-10). So **"roasted 3 days ago, light"
  deserves a longer bloom than "roasted 21 days ago, medium"**.
* Pre-ground coffee degasses within hours; if the user uses pre-ground, shorten the bloom.

### `bloomTemperature` (°C)
* Fellow default: same as brew temperature [F].
* Hotter bloom (+1-2 °C over the pulses) front-loads extraction of dense light roasts. Equal or lower for delicate
  naturals/anaerobics, where an aggressive start tends to pull ferment/alcohol notes and early harshness [H].
* Practical anchor: **bloom temperature = first pulse temperature + 0 to 1 °C** (the user's own profile uses +1).

---

## 5. Pulses: `ss/batch PulsesEnabled`, `…Number`, `…Interval`

### What a pulse does
After the bloom the machine delivers the remaining water in `Number` equal portions, `Interval` seconds apart. Each
portion hits the bed, agitates it, and refreshes the concentration gradient (when the slurry is already saturated with
dissolved coffee, extraction slows; letting the bed drain a little and adding fresh water speeds it up again) [E].
Evidence from pour-over practice: pulses give a modest bump in extraction yield (~1-3 percentage points in home setups),
cleaner/brighter cups with more acidity and less body than one continuous pour [E]. Fellow states the principle plainly
[F]: **more pulses + higher temperature = higher extraction.**

### `…PulsesEnabled`
* `true` for nearly everything (the whole point of the Aiden). `false` = a single continuous delivery; use for
  body-forward dark roasts, or when the user wants a fuller, rounder, less clear cup [E].
* When `false` keep `…Number: 1`, one temperature in the list, and a valid interval (the lazyaiden repo's
  `light-roast-batch` sample does this: `ssPulsesEnabled: false`, `ssPulsesNumber: 1`, `ssPulseTemperatures: [98.0]`).

### `…PulsesNumber` (1-10)
| Count | Effect | Fits |
| --- | --- | --- |
| 1 | One pour. Fullest body, lowest clarity, lowest extraction | Dark roasts, very easy to extract, big batches (Fellow's default for batch is 1) |
| 2-3 | Balanced. Fellow's default for single-serve is 3 | Most coffees; medium roasts |
| 4-5 | More extraction, more clarity; smaller pours | Light roasts, dense/high-altitude, washed |
| 6-7 | High agitation, long gentle extraction. Used by Fellow's own Ethiopia recipe (7 pulses, 205→200 °F) | Very light, very dense, sour-prone coffees |
| 8-10 | Diminishing returns; each pour is tiny; brew time balloons | Experiments only |

Natural/anaerobic/very fruity coffees: prefer **fewer** pulses (2-3) to avoid pulling harsh, over-fermented, or tannic
notes [H]. Evidence for per-process pulse counts is weak; the Equator recipes reason the same way ("reduced agitation
preserves subtle profiles").

### `…PulsesInterval` (seconds, 5-60)
* Fellow default 23 s [F]; community range 20-30 s; large batches up to 40 s (Equator: 3 × 40 s) [E].
* **Longer interval**: bed drains more between pours, more total contact time, more extraction, less agitation per
  second. **Shorter interval**: bed stays wetter, faster overall brew, slightly less extraction (a home-barista
  tip is "grind coarser, lower temperature and/or shorter interval" to fix over-extraction) [E].
* Interval is not "free": total pour time ≈ bloom + (pulses × interval) + drawdown. A very slow brew (> ~5-6 min
  total in single serve) risks over-extraction; a very fast one risks under-extraction.
* Batch beds are deeper and drain slower: **use about +10 s over the single-serve interval** [H]; this is what the user's
  real profile does (20 s single, 30 s batch) and Equator's large-batch recipe (40 s) agrees. See §8b.

### Single-serve vs batch
Both sets exist in the same YAML. The brewer uses the single-serve pulse set for small brews (up to ~450 ml) and
the batch set above.
The two should usually be **coherent but not identical**: same temperature curve shape; batch = same or slightly
more pulses OR longer interval to compensate for the deeper bed, never radically different. If the user says "I only
make X", still fill both sets sensibly so the profile does not surprise them later.

---

## 6. Pulse temperatures: `ssPulseTemperatures`, `batchPulseTemperatures`

### What per-pulse temperature does
Different compounds dissolve at different speeds: acids and fruity notes early, sugars mid, heavy bitter/astringent
compounds late and more with heat [E, widely repeated, but the UC Davis study shows that, once TDS and PE are held fixed,
the temperature itself does not change flavour, so the real mechanism is "temperature moves extraction yield"].
So per-pulse temperature is a way of **spending extraction early (hot) and being gentle late (cool)**.

### Patterns
| Pattern | Example (°C) | Intent | Fits |
| --- | --- | --- | --- |
| Flat | 96, 96, 96 | Simple, predictable | Medium roasts, first try |
| Gentle decline | 96, 95.5, 95 (the sample `morning-v60`) | Slightly less extraction at the end | Most light/medium |
| Strong decline | 96.5, 95, 93, 91 | Front-load sweetness, protect against late bitterness | Light roasts that turn bitter/drying at the end; sweet-focused |
| Fellow Ethiopia | 96, 95.5, 95, 94.5, 94, 93.5, 93.5 (205 → 200 °F) | 7 hot-to-slightly-cooler pulses for floral washed | Delicate washed light roast |
| Hot then cool | 96, 96, 87 | Full extraction then stop bitterness | Dark roasts, bitter-prone |
| Rising | 90, 92, 94 | Rare; gentle start, hotter finish to push under-extracted coffee | Dense, sour coffees where a cool start is wanted (rare) |

Heat-loss note: manual pour-over loses heat in the open bed, so people over-heat the kettle to compensate [E]. Aiden
recipes are written as the brewer's set temperatures, so copy them as-is and **do not add kettle-style compensation** [H].

### Rules
* **List length must equal `…PulsesNumber`.** Neither this project's validator nor the reference clients check it,
  and how the brewer handles a mismatch is undocumented, so never produce one. If you change the count, rebuild the list.
* Temperatures are on a 0.5 °C grid, 50-98.5 °C. A Fahrenheit recipe must be converted (table in §8) and snapped.
* Don't make large single-step drops (> 4 °C between adjacent pulses) unless intentionally using the "hot then cool" pattern.

### Temperature by roast level (°C) [H, built from Fellow presets + roaster guidance]

| Roast | First pulse / bloom | Last pulse | Why |
| --- | --- | --- | --- |
| Light | 94-98 | 92-96 | Dense, low porosity; resists extraction; needs heat |
| Medium-light | 93-96 | 91-95 | |
| Medium | 92-96 (Fellow preset ≈ 96) | 90-94 | Balanced |
| Medium-dark | 89-93 | 87-91 | More soluble; heat pulls bitter/ashy |
| Dark | 85-91 (Fellow's dark preset reportedly bloom hot, pulses ≈ 85) | 84-88 | Cell walls broken down, melanoidins extract easily; cooler avoids smoke/ash |

Sources for the ranges: Fellow presets (secondary), Rao ("high temperature is fine, 93-96 °C"), roaster guides (light
195-205 °F, dark 190-200 °F). The ranges overlap on purpose.

---

## 7. How bag information maps to settings

Everything the skill reads off a bag, why it matters, and the direction to move. These are **[H]** unless noted.
Use them to pick a starting point inside the ranges above. Start in the middle of the range and move by the signs below.

### Roast level (most important, [S] + [F])
Dark: more soluble, more porous, much more melanoidin (~97 mg/g vs ~29 mg/g medium, a 2023 study as quoted by a roaster blog [E]), extracts easily, so cooler
water, fewer pulses, higher ratio, shorter/lower bloom. Light: dense, resists extraction, so hotter water, more
pulses, 1:3 bloom, longer bloom, finer grind. If the bag does not state the roast level, infer from description
("filter roast", "omni", "espresso roast", flavour notes of fruit/floral vs chocolate/smoke) and **say which you assumed**.

### Process
| Process | Typical flavour | Direction [H] |
| --- | --- | --- |
| Washed | Clean, bright, floral, citric | Top of the temperature range, 3-5 pulses (anchors use 2-7), 1:3 bloom, ratio 16.5-17. Gives the clarity this process is prized for |
| Natural/dry | Fruity, jammy, heavier, higher sugars, can be fermenty | −1 to −3 °C, 2-3 pulses, shorter bloom (30-35 s), ratio 16-16.5, optionally coarser grind |
| Honey/pulped natural | Sweet, round, mid | −1 °C vs washed; 3 pulses |
| Anaerobic/carbonic/co-ferment | Intense, winey, funky, very soluble | −2 to −4 °C, 2-3 pulses, ratio 16.5-17 to dilute intensity, cooler last pulses. Easy to over-extract into hard fermented bitterness |
| Wet-hulled (Sumatra etc.) | Earthy, heavy, low acid | Treat like medium-dark: cooler, 2-3 pulses, ratio 16 |
Roaster guides give absolute numbers (washed 93-96 °C, natural 88-92 °C, anaerobic 91-93 °C) [E]. They are lower than
Aiden-community numbers because they come from manual pour-over; use the *relative* offsets above rather than the absolutes.

### Altitude and density [E]
High (> 1,800 masl) coffees are denser: harder to extract, so +1-2 °C, one more pulse, finer grind, 1:3 bloom. Low
altitude (< 1,200 masl, e.g. many Brazils) is softer: −1 °C, fewer pulses, ratio 15.5-16.

### Variety [E, weak]
Geisha, SL28/SL34, Pink Bourbon, Heirloom Ethiopian: delicate, aromatic. Lean on gradual temperature decline and gentle
agitation to preserve florals, ratio 16.5-17. Robusta/Catuai/Mundo Novo/Caturra in low-cost blends: sturdy, extracts easily, fine at
a lower temperature and ratio 15.5-16.

### Roast date / freshness [H, built on the F/E degassing facts in §4]
Bloom duration is the lever: < 7 days off roast → 40-60 s; 7-21 days → 30-40 s; 21-45 days → 25-35 s; older → 20-30 s and
consider a hotter first pulse to compensate for lost solubles. Coffee < 3 days off roast is hard to extract evenly regardless
of the bloom: say so.

### Tasting notes on the bag (use for ratio and temperature slope)
* Fruit/floral/tea-like/citrus notes: want clarity → ratio 16.5-17, declining temps, more pulses.
* Chocolate/nut/caramel/body notes: want sweetness and body → ratio 15.5-16, flatter temps, 2-3 pulses.
* Smoky/roasty: avoid pushing extraction; cooler and fewer pulses.

### Brewing volume
Single-serve (≤ 450 ml, one or two mugs) vs batch (451-1500 ml). Larger volumes want longer intervals and a coarser grind
(Fellow: finer end of medium for single serve → coarser medium for 5-10 cups) [F].

### Grind direction (not in the YAML, always state it) [F, E]
The Aiden brews with whatever grind you give it, and recipes always come with grinder settings. Examples from Fellow's
Ethiopia Danche recipe (washed light, 1:17): single-serve Ode Gen 2 **3.2**, Ode + SSP 4.2, Opus 5.2; batch Ode Gen 2 **4.2**,
Ode + SSP 5.2, Opus 6.2. Rule of thumb: **batch is about one full step coarser than single-serve; light roasts finer than
dark; naturals/anaerobics slightly coarser than washed.** Always ask which grinder the user has and tell them to
*start there and taste*.

---

## 8. Reference tables

### °C ↔ °F (snap to 0.5 °C grid)
| °F | 185 | 190 | 195 | 198 | 200 | 202 | 204 | 205 | 206 | 208 | 210 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| °C | 85.0 | 87.8 → 88 | 90.6 → 90.5 | 92.2 → 92 | 93.3 → 93.5 | 94.4 → 94.5 | 95.6 → 95.5 | 96.1 → 96 | 96.7 → 96.5 | 97.8 → 98 | 98.9 → **98.5 (max)** |

The brewer's top setting is 98.5 °C; Fellow's own "210 °F" light roast preset therefore tops out at 98.5.

### Anchor profiles (known-good starting points, converted to the YAML grid)
| Source | Coffee | ratio | bloom (ratio / s / °C) | pulses | interval | pulse temps (°C) |
| --- | --- | --- | --- | --- | --- | --- |
| Fellow preset (secondary) | Light | 17 | 3 / 45 / 98.5 | 3 | 23 | 98.5 ×3 |
| Fellow preset (secondary) | Medium | 16 | 2 / 30 / 96 | 3 | 23 | 96 ×3 |
| Fellow preset (secondary) | Dark | 16 | 2 / 30 / 98.5 *(unverified)* | 3 | 23 | 85 ×3 *(unverified)* |
| Fellow Ethiopia Danche | Washed light | 17 | 3 / 45 / 96 | 7 | 20 | 96, 95.5, 95, 94.5, 94, 93.5, 93.5 |
| Brewshare Kenya Gicherori AA | Washed light | 17 | 3 / 45 / 96.5 | 2 | 25 | 97.5, 96 |
| Equator washed SO | Light | 16 | 3 / 35 / 95.5 | 2 | 30 | 95.5 ×2 |
| Equator natural SO | Light | 16.5 | 3 / 30 / 94.5 | 3 | 25 | 94.5 ×3 |
| Equator blend | Med-dark | 15.5 | 2.5 / 45 / 93.5 | 3 | 40 | 93.5 ×3 |
| Community light roast | Light | 17 | 3 / 30 / 94.5 | 4 | n/a | 93.5, 93.5, 88, 88 *(from °F: 200, 200, 190, 190)* |
| Community "rescue" | Bitter medium | n/a | n/a | 3 | n/a | 91.5, 89.5, 87 |
| **Ali's own `Ali Ethiopian Light`** (19 g dose; real, tasted) | Probably Ethiopian (Sidama); roast/process not recorded, profile shape suggests light | 16.5 | 3 / 45 / 96 | 3 ss / 3 batch | ss 20, batch 30 | 95, 93, 91 (both sets) |
| Repo sample `morning-v60` | n/a | 16 | 2 / 35 / 96 | 3 | 20/25 | 96, 95.5, 95 |
| Repo sample `light-roast-batch` | light | 15.5 | 3 / 45 / 98 | batch 4 | 20 | 98, 97.5, 97, 96.5 |

---

## 8b. The user's own style (calibration, from a real profile they brew)

File: `user-profiles/ali-ethiopian-light.yaml`. Read this as **their taste**, which beats generic rules when they conflict.

* **Ratio 16.5**: between balanced and clean; they like some clarity but not tea-thin.
* **Bloom 3.0 / 45 s / 96 °C**: they bloom generously and long. Default their bloom to 3 and 40-45 s for light/medium-light;
  shorten only for dark or stale coffee.
* **Declining pulses in 2 °C steps (95 → 93 → 91)**, bloom 1 °C above the first pulse. This is a "strong decline" pattern:
  hot early for sweetness and acidity, cool late to avoid bitterness. Default to this shape (step 1.5-2 °C) for light
  roasts instead of flat curves.
* **3 pulses**: moderate agitation, not the 7-pulse Fellow style.
* **Batch interval = single-serve interval + 10 s (30 vs 20)**, with **identical temperatures** in both sets and the same
  pulse count. §5 uses the same +10 s rule.
* Title pattern: `<Person> <dose>grams <Coffee>`: dose in the title; **19 g** is a typical single-serve dose for them.
  Ask for their usual dose if it matters for the title, otherwise omit.

Caveat: this is one profile for one coffee. It shows preferences, not a universal rule: a dark roast or natural should
still move cooler and shorter.

---

## 9. Taste → fix (the feedback loop)

The skill should ask for one-word feedback after the first cup and move **one lever at a time**.

| Cup tastes | Diagnosis | First change | Second change |
| --- | --- | --- | --- |
| Sour, sharp, thin, salty, grassy | Under-extracted | +1-2 °C on all pulses **or** +1 pulse; lengthen bloom +10 s | Grind finer one step. *(Don't lower the ratio for sourness: it adds strength but slightly lowers extraction.)* |
| Bitter, astringent, drying, ashy, smoky | Over-extracted | −1.5-2 °C; cool last pulses (−3 °C); shorten bloom | Grind coarser; −1 pulse; ratio +0.5 to dilute |
| Flat, dull, "nothing there" but not sour | Lack of aromatics/extraction | Longer bloom; +1 °C; 1:3 bloom | Check freshness: old coffee can't be fixed |
| Hollow/watery | Too dilute or under-extracted | Ratio −0.5 to −1 | +1 pulse |
| Heavy, muddy, harsh | Too strong or over-extracted | Ratio +0.5-1 | Fewer pulses; coarser |
| Sour **and** bitter at once | Uneven extraction (channeling, fines, too-fresh) | Longer bloom (+15 s), 1:3 bloom, more pulses with shorter interval | Fix grind/filter, rest coffee |
| Brew is sweet but muted/boring | Ok extraction, too gentle | Ratio −0.5; hotter first pulse | |
| Fine until the last sip, then dry | Late over-extraction | Cooler final pulses | Fewer pulses |

Fellow-specific [F]: "sour/flat → lengthen bloom; bitter → shorten bloom". Grind advice from forums [E]: "coarser grind,
lower temperature and/or shorter interval" for over-extraction.

---

## 10. What is well supported, what is not

| Claim | Support |
| --- | --- |
| Strength (TDS) and extraction yield determine flavour; temperature alone has little effect at fixed TDS/PE | **[S]** strong |
| Higher TDS → bitter/smoky; lower TDS → fruity/floral; high PE → ashy/brown; low PE → sour/citrus | **[S]** strong |
| Temperature, bloom, pulses raise/lower extraction yield (Fellow) | **[F]** |
| Dark roasts extract more easily; light roasts are dense | **[S]** (porosity, melanoidins) |
| Roast date → degassing → bloom length | **[F]**, **[E]** |
| Pulses give a modest extraction/clarity gain over a continuous pour | **[E]** (no rigorous Aiden study) |
| Per-process temperature offsets (natural −2 °C etc.) | **[H]** weak: roasters differ |
| Altitude/variety offsets | **[H]** weak |
| Declining pulse temperatures help sweetness | **[E/H]**: plausible via extraction, no controlled study |
| Dark preset 85 °C pulses; Fellow help-centre numbers | *unverified* (403) |

**Limits to say out loud when presenting a profile.** It is a starting point, not a verdict: bag info cannot reveal
grind, water hardness, grinder burr quality or how the roaster actually developed the coffee. Always include a grind
suggestion and the "taste → fix" loop.

---

## Sources

* Batali et al., brew temperature at fixed strength/extraction has little sensory impact: https://pmc.ncbi.nlm.nih.gov/articles/PMC7536440/
* Hot vs cold brew chemistry by roast and temperature: https://pmc.ncbi.nlm.nih.gov/articles/PMC7404565/
* Guinard et al. 2023 Brewing Control Chart (abstract only): https://ift.onlinelibrary.wiley.com/doi/10.1111/1750-3841.16531
* Cotter et al. 2021 consumer preferences (abstract only): https://ift.onlinelibrary.wiley.com/doi/10.1111/1750-3841.15561
* Fellow, Understanding degassing: https://fellowproducts.com/blogs/learn/understanding-degassing-is-fresh-best
* Fellow, Ethiopia Danche Aiden recipe: https://fellowproducts.com/blogs/brew-talks/fellows-take-on-organic-ethiopia-danche-by-wonderstate-brew-recipe
* Seattle Coffee Gear Aiden guide (defaults, volumes, dose limits): https://www.seattlecoffeegear.com/pages/product-resources/fellow-aiden-coffee-maker-product-guide
* Equator Aiden recipes: https://www.equatorcoffees.com/blogs/guides/fellow-aiden
* Brewshare community database: https://brewshare.coffee/ and Brew Commons generator: https://www.brewcommons.com/
* Fellow help centre (pre-installed profiles, temperature) was blocked (403); numbers come via secondary sources.
* Roaster/educator pages for roast, process, altitude, bloom, freshness heuristics (Clive Coffee, Podium, Achilles, Phoenix,
  Door County, Coffee Crafters) and the Home-Barista Aiden threads.
* Scott Rao positions via secondary summaries (bloom about 3× the coffee weight, hot water 93-96 °C).
* Community client documenting the API fields: https://github.com/9b/fellow-aiden
