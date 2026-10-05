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

## Next

Ground truth (a full scroll of a known stash), then a matcher in Rust in
`arclens-vision` with fixture tests, combining the icon score with the
numeral and the category glyph.
