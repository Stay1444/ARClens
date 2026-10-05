# Findings: recognising stash slots by their icons (2026-10-05)

**Question:** can ARClens tell what is in each stash slot without the
player hovering it? The grid shows only an icon, a rarity colour, a
category glyph, and a quantity (`×60`) or tier numeral (`I`–`IV`). No names.

**Data:** fixtures `inventory_stash.jpg` and `stash_none_1.jpg`
(2560×1440, 24 slots each) against the 588 item images in
RaidTheory/arcraiders-data (`images/items/*.png`, 256×256, transparent
background). Prototype in Python (numpy/scipy), not committed.

## What the icons look like (verified)

- Slot icons are **the same 3D models** as the dataset images, but rendered
  from a slightly different camera angle, smaller, on a navy background
  with a rarity-coloured arc in the bottom-left corner. Pixel-exact
  template matching won't work.
- At 2560×1440 a slot is ~124 px with a ~139 px pitch; the stash grid is
  four columns wide.
- The bottom ~18 % of a slot is a bar with the category glyph (left) and
  the quantity or tier numeral (right).

## Method that worked best

1. Crop the slot, drop the bottom bar and a 4 % border.
2. Foreground mask: difference from a 31-px median-filtered background,
   OR gradient magnitude above a threshold; drop strongly saturated
   (rarity-coloured) pixels; close, fill holes, keep the largest blob.
3. Square-crop to the mask's bounding box, resize to 32×32.
4. Same for each dataset image (its alpha is the mask).
5. Score = mask IoU − 2 × mean colour difference inside both masks; also
   try the slot mirrored; take the best.

## Results (verified on the two fixtures, unverified accuracy)

| Kind | Result |
|---|---|
| Ammo (heavy, light, launcher), energy clip | Correct and clear: top score 0.3–0.6, the runner-up below −0.2 |
| Shields | Medium shield found; heavy shield ranked second behind "ruined riot shield" |
| Augments | Family usually right (tactical, looting), mark often wrong; a looser mask did better on these than on weapons |
| Weapons | Plausible family (Vulcano, Osprey, Renegade, Torrente, Anvil, Ferro, Rattler, Burletta) with low margins; not reliable yet |

Accuracy is not measured: there is no ground truth for most slots yet.

## Limits found

- **Tiers share one image** (`anvil_i` … `anvil_iv`, and the blueprint).
  The tier must come from the numeral in the slot's bottom bar; blueprints
  probably need their own visual cue (unverified: no blueprint in the
  fixtures).
- Weapons are long grey silhouettes on a dark background; the family
  margins are small. The category glyph (bottom-left) can narrow the
  candidates to one weapon class.
- The stash header already shows the stash's total value (`183,678`), so
  the audit's value is the per-item advice, not the total.

## Ground truth and the Rust matcher (verified, same day)

- **Ground truth:** a second recording hovering each slot. The tooltip
  names (read by our OCR) were paired with the same slots in the scroll
  recording, so the labelled crops have **no cursor in them** (the hover
  frames show KDE's or the game's pointer over the slot, which must not
  be learned). 42 slots, 5 frames: `crates/arclens-vision/tests/fixtures/stash/`.
- **Rust port** (`arclens_vision::{stash_slots, slot_features, IconIndex}`):
  on those 42 slots the right item *family* (tiers and blueprints share
  an image) comes first **88 %** of the time, in the top three **95 %**.
  Misses: augment marks (Combat Mk. 2 vs Looting Mk. 1, Looting Mk. 2 vs
  Mk. 3), Heavy Shield vs "ruined riot shield", Hornet Driver vs its
  damaged variant.
- **Tier numerals** (`slot_tier`) read by counting stems, with a shape
  test for the V of "IV": all 9 tiered slots right, no OCR.
- **Stack sizes** (`read_badge`) by OCR of the slot's corner; OCR drops a
  lone "1" after the "×" and sometimes reads "×" as "?", both handled.
- The grid's rows come from the first column's outline (tooltips open to
  the right, so it's never covered); the gap between rows can be as
  bright as ~50, so outlines need ≥ 55.

## Filter tabs (verified on screenshots, same day)

- The stash has ten filter tabs in a column left of the grid (all,
  augments, shields, weapons, …), discs centred at x = 133,
  y = 393 + 67·i (1440p). The selected one is a white disc (rim ≥ 240),
  the rest dark grey (≈ 65): `arclens_vision::stash_filter` reads it,
  tested at 2000×1125 and 2560×1440 (`fixtures/stash/filter_*.jpg`).
- On a filtered tab the header shows only that tab's slot count ("5",
  no "/280"), and the grid is the same 4-column layout, padded with empty
  slots. Only the "all" tab is stitched into a scan; filtered tabs get
  tags slot by slot.

## Next

In the app: learn exact slot images from hovered tooltips (after the
cursor has left the slot), merge rows across scrolling, and save scans.
