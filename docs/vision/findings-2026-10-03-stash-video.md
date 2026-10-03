# Findings: stash hover video (2026-10-03)

**Source:** maintainer's screen recording of the main-menu stash/loadout
screen. 2560×1440, 60 fps, VP9, 26 s, ~0.8 Mbit/s (Spectacle default).

## What the tooltip looks like (verified)

- **Panel:** a large, **opaque cream / off-white rectangle** with dark text.
  It sits on a dark, desaturated inventory UI, which makes it the
  highest-contrast object on screen.
- **Header:** a small "ACTIONS" tab, then a coloured chip with type and rarity
  (e.g. blue "LMG · RARE", purple "AUGMENT · EPIC").
- **Name:** **bold, dark, uppercase, one line**, at the top of the panel
  ("TORRENTE II", "OSPREY II", "VULCANO I", "MEDIUM AMMO", "ENERGY CLIP",
  "SHIELD RECHARGER", "COMBAT MK. 3 (AGGRESSIVE)").
- **Body:** description, stat table, upgrade modifiers, "RECYCLES INTO" with
  small icons.
- **Footer:** weight (e.g. `14.25`) and **sell value** (e.g. `23,569`).
- **Placement:**
  - top-aligned with the hovered slot, opening to the **right** of it;
  - flips to the **left** near the right screen edge (quick-use and backpack
    slots).
- **Behaviour:** appears almost instantly on hover and disappears when the
  cursor leaves the slot or is over empty space.

Text stays crisp even at this bitrate. Compression is not a problem for
reading the name.

## Name ↔ dataset match (verified)

All 7 names seen match a RaidTheory item name exactly after upper-casing:
`torrente_ii`, `osprey_ii`, `vulcano_i`, `medium_ammo`, `energy_clip`,
`shield_recharger`, `combat_mk3_aggressive`.

## Value discrepancy (verified, important)

| Item | Tooltip sell value | Dataset `value` |
|---|---|---|
| Torrente II | 23,569 | 10,000 |
| Osprey II | 18,431 | 10,000 |

Tooltip values for weapons are higher than the dataset's (upgrades and
durability presumably factor in, **unverified**). For weapons the tooltip is
the more accurate source. If we read the footer value, the verdict can use the
real number.

## Consequences for the design

1. **Identify by reading the name; drop icon matching.**
   - The tooltip holds no large item icon; the icon is in the slot under the
     cursor.
   - The name is high-contrast, single-line text with a closed vocabulary of
     about 580 entries. Recognise the line, then fuzzy-match it against the
     catalogue (`ItemSearch`).
   - This tolerates recognition errors, needs no reference icons, and is
     language-portable through the dataset's localised names.
2. **Detect the tooltip without OCR:**
   - threshold for the cream colour on a downscaled frame;
   - find the largest bright axis-aligned rectangle;
   - check its aspect ratio and size;
   - the name line is a fixed offset below the coloured chip.
3. **Place our card relative to the detected rectangle.** Under it if there's
   room, otherwise beside it on the side away from the hovered slot. No
   cursor position is needed for this.
4. **Read the footer value** (digits only) to improve the verdict for
   weapons.

## Raid and trader footage (verified, same day)

- **Raid backpack:**
  - same cream tooltip;
  - the header tab reads "PING ITEM" / "REQUEST AMMO" / "REQUEST SHIELD
    RECHARGER" instead of "ACTIONS";
  - "SALVAGES INTO" instead of "RECYCLES INTO";
  - **the footer shows only weight, with no sell value.** In raid we rely on
    the dataset value.
- **Trader screen:**
  - a **persistent cream purchase panel** ("×25 LIGHT AMMO") is always shown;
  - hover tooltips appear beside it and can **touch or overlap it**, merging
    into one cream region;
  - its name has a quantity prefix (`×25`) that must be stripped before
    matching;
  - the selected item tile is also cream but small.
- **Colours (measured):**
  - tooltip body (248,232,216);
  - header tab (200,184,168), deliberately excluded;
  - name text ≤ 25 per channel;
  - grey "COMMON" chip ~105;
  - coloured chips carry dark text too.

## Detector (implemented: `crates/arclens-vision`)

1. **Cream mask:** subsample every 4th pixel and threshold the body colour.
2. **Panels:** take connected components, keep the large ones.
3. **Merged panels:** split them where the left edge jumps by more than
   2.5 % W. A split panel's width is clamped to the fixed tooltip width
   (0.353 H).
4. **Header tab:** trim it off by keeping only rows that are ≥ 60 % cream.
5. **Name:** the first band of near-black rows whose background (across its
   own ink extent) is ≥ 70 % cream. This skips chips. A following band of
   similar height is treated as a wrapped second line.

Results on the 18 committed fixture frames:
- all 17 name lines found;
- zero false panels.

Speed: **1.4–6.5 ms per 1440p frame** on one core in a release build.

## Name recognition (implemented)

- **Engine:** [`ocrs`](https://github.com/robertknight/ocrs), pure Rust on the
  `rten` runtime.
  - Only its **recognition** model runs, on the detected line crops, so the
    detection model and any full-frame pass are skipped.
- **Result: 17/17 names read exactly** on the fixtures (opt-in test
  `crates/arclens-vision/tests/ocr.rs`).
- **Speed:** ~30–40 ms per one-line name and ~75 ms for two lines, on one CPU
  core (release build).
- **Known model weakness:** CTC decoding merges repeated characters, so
  "OSPREY II" comes out as "OSPREY I", and it drops the space in
  "GRIP I" → "GRIPI".
  - This matters because *Osprey I and Torrente I exist* as separate items.
  - Fix: count the bars of a trailing word made only of narrow full-height
    glyphs in the image itself, then rewrite the numeral.
  - Measured spacing at 23 px cap height: letter gaps 3–4 px, the gap between
    the bars of "II" 6 px, word gaps 12 px.
- **Matching:** `arclens_data::match_name` normalises the text (case,
  whitespace, trader `x25` prefix) and requires an exact match or ≥ 0.9
  similarity. All 13 distinct names map to the right dataset id.
- **Model licence:**
  - the ocrs models are trained only on HierText (CC BY-SA 4.0);
  - no separate licence for the weights was found (**unverified**);
  - the app **downloads** `text-recognition.rten` from the upstream URL at
    runtime and never bundles or redistributes it.

## Performance budget (why no GPU OCR)

The game is GPU-bound, and GPU inference would compete with it for every
frame. The CPU has headroom, and the work is small:

| Stage | When | Cost (1 core, release) |
|---|---|---|
| Capture | 4 fps idle, 10 fps for 3 s after tooltip activity | One BGRx→RGB conversion per used frame; surplus frames dropped |
| Find panels + name lines | every captured frame | 1.4–6.5 ms at 1440p |
| OCR | only for tooltips not seen this session (cache by crop fingerprint) | 30–75 ms, once per *new* tooltip |

- That is under 3% of one core while browsing, plus a short burst per new
  item, run on a low-priority background thread.
- Perceived latency: one capture interval (≤ 100 ms while browsing, ≤ 250 ms
  for the first hover after idling), plus ~35 ms for an unseen item.
- If needed, the capture rate can rise only while a tooltip is visible.

## First live run (2026-10-03, maintainer's machine)

- **Setup:** KDE Plasma 6 Wayland, 2560×1440, no scaling, ARC Raiders under
  `PROTON_ENABLE_WAYLAND=1`. Release build, "Detect items" toggled on.
- **Capture:** the portal and PipeWire delivered `BGRx` 2560×1440 frames on
  the first try.
- **Results:** 10 distinct items, all matched to the right id:
  - Looting Mk. 2, Free Loadout Augment, Combat Mk. 2;
  - Combat Mk. 3 (Aggressive), the two-line name;
  - Renegade IV, Tactical Mk. 2, Light Shield, Burletta I.
- **One OCR slip:** "LLIGHT SHIELD" was absorbed by fuzzy matching (0.92 ≥
  0.9). This is the case the threshold exists for.
- **Felt:** "a delay, but perfectly fine." The card appeared in the right
  place beside the game tooltip.
- **Observation:** the same item is sometimes reported 2–3 times while
  hovered, probably small changes in the detected rectangle between frames.
  Harmless, but it re-sends the card. Worth debouncing.

## Raid vs. menu, and the real value (implemented 2026-10-03)

- **The footer** is a darker beige bar (203,191,174) under the body, ~0.044 H
  tall. It has full-height cells:
  - `[weight]` in raid;
  - `[weight, value]` in the menu, stash and trader.

  A smaller stack-count cell ("80/80") is ignored by height.
- **The value** reads as e.g. "S 23.569": the coin icon becomes a letter, and
  the comma sometimes becomes a period. Keeping only the digits gives the
  value exactly on all 7 menu fixtures. Raid is detected on all 4 raid
  fixtures.
- **Why it matters:**
  - The game's value already includes durability (Combat Mk. 2 at 41/100
    sells for 800, not the dataset's 2,000) and upgrades (Torrente II 23,569
    vs 10,000).
  - In raid, items are *salvaged* (`salvagesInto`, usually fewer outputs),
    not recycled. Example: Hairpin I salvages into Metal Parts ×2, but
    recycles into Metal Parts ×2 + Rubber Parts ×1.
- **Still unknown:** the in-raid value of damaged items. The raid footer has
  no value, so the card shows the dataset's base value, labelled "SELL (BASE
  VALUE)". No simple durability formula fits both data points (2000 × 0.41 =
  820 vs 800 shown; weapons include upgrades), so we don't guess.

## Open questions

- **Crafting screens:** not yet seen.
- **Other resolutions and UI scales:** thresholds are mostly relative, but
  test 1080p and 4K.

## Fixtures

These frames haven't been committed yet (copyrighted game UI, large PNGs).
The plan is to commit small **crops of the tooltip and slot** to
`crates/arclens-vision/tests/fixtures/` through Git LFS once the crate
exists.
