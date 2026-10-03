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

## Open questions

- **Crafting screens:** not yet seen.
- **Other resolutions and UI scales:** offsets need to scale. Test 1080p.
- **OCR engine:**
  - candidates: `ocrs` (pure Rust, ONNX-free models) vs. Tesseract via FFI;
  - prefer pure Rust if accuracy on this font is good;
  - benchmark on crops from this video.

## Fixtures

These frames haven't been committed yet (copyrighted game UI, large PNGs).
The plan is to commit small **crops of the tooltip and slot** to
`crates/arclens-vision/tests/fixtures/` through Git LFS once the crate
exists.
