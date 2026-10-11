# Research: Frozen Trail (update 2.0)

_Researched: 2026-10-11, three days after release. Everything here comes
from news coverage and search snippets; the official patch notes page
(arcraiders.com/news/frozen-trail-2-0-update) and the Embark help centre
could not be fetched from the dev environment. Treat it all as
**unverified** until checked in game._

## What changed (unverified)

- **Released 2026-10-08** as update 2.0, "Frozen Trail".
- **New map: Pendola Pass.** A snowy valley. Areas named in guides:
  Almatera Village (with the fallen "Emperor" ARC, explorable inside with
  an Emperor Gateway Conduit), Freight Terminal (Railyard, Commissary
  Supermarket), Northern Tracks. A "Ballistic Gondola" extraction. New
  players unlock it after 25 raids. Guides disagree on how many areas it
  has (four districts vs nine areas).
- **Conditions**: _Redirection_ (Pendola Pass only; a Transmitter pulls
  Harvester payloads to the player; switches Flash Freeze off) and the
  _ARC Frigate_ operation (a flying ARC ship to board; sources disagree on
  which maps it rotates through). _Flash Freeze_ is a map mechanic on
  Pendola Pass (a cold storm), not a scheduled condition. Night Raid also
  runs there.
- **New ARC**: Bully, Skulker, Hydra (one outlet says "Skoraka").
- **New weapons**: Bantam (heavy-ammo revolver), Stiletto (light ammo).
  **Amplified weapons** upgrade system.
- **New gear**: Grappling Hook, Tether Launcher, Yank Grenade, Basic and
  Advanced Cameras, Mountaineer's Detector, Banjo, Harmonica.
- **Progression**: revamped skill tree; **Outposts** with a **Research
  Bench** spending Research Points (Bantam and Stiletto research at level
  1, 3 000 points each, per one guide); a 60-level **Reward Pass**
  replacing the deck system; **Cred replaced by Reward Points**; 100
  furniture designs; two extra stash upgrade tiers; 22 new quests (named
  in guides: "Tracks of the Beast", "Belly of the Beast", both Shani's;
  Juanito, a Nomad trader, and Posh, a combat-challenge vendor, are new
  quest givers).
- **Balance**: Snap Hook range 20 → 18 m; Anvil moved from green to blue
  rarity.
- **Known issues** at launch: "Picking up the Pieces" can't be completed
  on Pendola Pass; a cinematic may not play after "Sizing Things Up".

Sources (2026-10-11): [Forbes](https://www.forbes.com/sites/erikkain/2026/10/09/arc-raiders-frozen-trail-patch-notes-new-map-weapons/),
[Sportskeeda](https://www.sportskeeda.com/esports/news-arc-raiders-frozen-trail-patch-notes-october-8-2026-pendola-pass-frigate-amplified-weapons),
[Beebom](https://beebom.com/arc-raiders-frozen-trail-update-patch-notes/),
[KeenGamer](https://www.keengamer.com/articles/guides/arc-raiders-frozen-trail-2-0-patch-notes-all-changes/),
[arcstatus.com](https://arcstatus.com/arc-raiders-frozen-trail),
[Embark help centre](https://id.embark.games/arc-raiders/support/faq/282-2-0-frozen-trail-update),
[gamingpromax (quests)](https://gamingpromax.com/arc-raiders-frozen-trail-new-quests-guide/),
[altchar (areas)](https://www.altchar.com/guides/arc-raiders-all-areas-on-the-new-pendola-pass-map-aFAKt9m7hwuj).

## Data sources after the update (checked 2026-10-11)

- **RaidTheory/arcraiders-data** `main` is still at 2a4abeb (2026-08-18,
  update 1.42). No 2.0 items, quests or Pendola Pass images yet. The only
  newer branch is an unmerged PR ("Frost Update All Assets", 2026-08-22),
  which predates the release. ARClens picks the update up by itself when
  RaidTheory merges it.
- **MetaForge** lists Pendola Pass on its map pages and in its map
  bundle. Its `/items` and `/quests` endpoints (shape recorded in
  Getty/p5-www-metaforge's fixtures, 2026-10-03) now fill what RaidTheory
  lacks: items and quests it added after RaidTheory's last update, and
  where items drop (`loot_area`, `dropped_by`). See `metaforge_db.rs`.
  metaforge.app is blocked from the dev environment, so the live 2.0 data
  itself is **unverified**.
- **Per-item spawn positions**: no API or dataset has them (searched
  2026-10-11: RaidTheory, MetaForge, ardb.app, Mahcks' mirror,
  ARC-vault/arc-vault-data, a Kaggle dump of ardb, Roxban's crowd-sourced
  blueprint atlas, which has no API). What exists: MetaForge markers that
  _are_ items (plants, pickups), their containers' `lootAreas`, and each
  item's loot-area types. ARClens combines those ("Show on map").

## What ARClens does with it

- Pendola Pass is a known map (`pendola-pass`): title OCR, names, events.
  Its place-name labels and map image are missing (a new HAR of
  MetaForge's map page and RaidTheory tiles are needed).
- Condition bits on Pendola Pass: No Event (0) from the pre-release
  bundle; Night Raid assumed bit 7 as on every other map (**unverified**).
  Redirection and Frigate have names but no bit: while they run, every
  marker shows.
- Cred: vendor costs in RaidTheory's 1.42 data still say `creds`. Whether
  2.0 vendors price in Reward Points is **unverified**.
