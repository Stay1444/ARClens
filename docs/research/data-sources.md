# Research: game data sources

_Last researched: 2026-10-03. Re-verify before relying on anything marked
**unverified**; community APIs change without notice._

## TL;DR

| Need | Source we use | Why |
|---|---|---|
| Items, recycling, quests, workshop, projects | **RaidTheory/arcraiders-data** (MIT, static JSON) | Licensed, offline-friendly, no rate limits, versioned in git |
| Map event schedule (timers) | **MetaForge** `events-schedule` (planned) | Only public machine-readable schedule; requires attribution |
| Map markers | **MetaForge** `game-map-data` (planned) or our own hand-placed set | Nobody has datamined world coordinates; all marker sets are crowd-sourced |
| Player stash / quest progress | _Not planned_ | Only reachable through Embark's private, reverse-engineered API |

## Embark (official)

- **There is no official public API**: no developer portal, no OAuth
  programme, no documented endpoints. At launch Embark said there are "no
  current plans" to support external APIs or sites
  ([fandomwire](https://fandomwire.com/arc-raiders-has-no-current-plans-to-support-external-sites-to-guide-players-in-the-game/)).
- The game client talks to a private backend
  (`api-gateway.europe.es-pio.net/v1/pioneer/...`, bearer token from the
  `embark-pioneer` OAuth client). It has been reverse-engineered
  ([baschny/arcraiders-raider-tools](https://github.com/baschny/arcraiders-raider-tools/blob/HEAD/docs/specifications/Embark-API.md)).
  MetaForge's and ARCTracker's "sync your stash" features use it. The retired
  ARCTrackerSync captured tokens by **sniffing the game's network traffic**.
  **ARClens does not use this API.** It is unofficial, needs a user token, and
  sits outside any terms we can point to.
- MetaForge is **not** partnered with Embark. Its API page says it is "not
  affiliated with, endorsed by" Embark.

## RaidTheory / arcraiders-data (primary source)

- Repo: <https://github.com/RaidTheory/arcraiders-data>. This is the team
  behind arctracker.io. **MIT.** Game content is © Embark.
- Layout:
  - `items/<id>.json` (~580 items);
  - `hideout/<station>.json`;
  - `quests/<id>.json`;
  - `projects.json`;
  - `maps.json` (id, name, image URL only, **no markers**);
  - `map-events/map-events.json` (event *types* and icons, **no schedule**).
- Localised strings are `{lang: text}` objects, sometimes plain strings.
  `arclens-data::raidtheory` handles both.
- Attribution requested: link to the repo and arctracker.io. The app
  displays the source in its footer.
- Loader: `crates/arclens-data/src/raidtheory.rs`. Download:
  `crates/arclens-data/src/download.rs` (GitHub tarball, JSON only).

## MetaForge

- Closed source, with a public unauthenticated API at
  `https://metaforge.app/api/arc-raiders/...` and `https://metaforge.app/api/game-map-data`.
- Endpoints: `items`, `arcs`, `quests`, `traders`, `event-timers`,
  `events-schedule`, `game-map-data`.
- **Terms** (from <https://metaforge.app/arc-raiders/api>):
  - attribution plus a link to metaforge.app/arc-raiders for public projects;
  - contact them on Discord before any paid or monetised use;
  - unpublished rate limits;
  - "endpoints may change or break without warning", so **cache everything**.
- `events-schedule` response (from a recorded fixture in
  [Getty/p5-www-metaforge](https://github.com/Getty/p5-www-metaforge)):

  ```json
  {"data":[{"name":"Cold Snap","map":"Dam","icon":"https://cdn.metaforge.app/...","startTime":1767556800000,"endTime":1767564000000}],"cachedAt":1767556800000}
  ```

  Times are Unix milliseconds.
- `event-timers` is the static table the schedule is expanded from:
  `{name, map, icon, days[], times:[{start:"HH:MM", end:"HH:MM"}]}` in UTC.
- **Live check (2026-10-03, maintainer's curl dumps):**
  - `events-schedule` matches the documented shape and adds
    `"region":"europe"` at the top level: the schedule is per region.
    Icons are on **`static.metaforge.app`** (`…/arc-raiders/custom/*.webp`).
  - `game-map-data` for Dam: 7 662 records, all with `id, lat, lng,
    zlayers, mapID, category, subcategory, instanceName, added_by,
    behindLockedDoor, last_edited_by, updated_at, eventConditionMask,
    lootAreas, sourceID, routeID, community`. Categories: containers 5 434,
    nature 1 518, events 224, arc 194, locations 164, quests 128.
    `lat` 975–4 293, `lng` 2 455–5 573.
  - **No place-name labels** in this table: `instanceName` is the object
    ("Generator", "Locker") or a free note ("On top of Pipeline Tower").
    The Labels/Zones layers of MetaForge's map must come from another
    table or endpoint (unknown yet), which label anchoring needs.
  - Some subcategories mark spawn **areas**, not spots: e.g. 884
    `raider_cache` records form dense blobs of candidate positions.
- **Map images** (2026-10-04): RaidTheory's map tiles,
  `images/maps/<map>/v2/{low,high}/{z}/{x}/{y}.webp` in its repository
  (MIT), fetched on demand from `raw.githubusercontent.com/RaidTheory/
  arcraiders-data/` (only that path is allowed in `ImageCache`) and cached
  on disk; the zoom-1 set is 16 small tiles per map. Alignment with
  MetaForge coordinates: `docs/research/map-images.md`.
- **What MetaForge's own map page loads (2026-10-03, maintainer's HAR of
  `metaforge.app/arc-raiders/map/dam`):**
  - `events-schedule?region=europe` (and `&includeAll=true`): the
    **`region` query parameter is confirmed**.
  - **Region ids** (same capture, MetaForge's JS bundle, read 2026-10-04):
    `["europe","north-america","brazil","east-asia","oceania"]`, labelled
    Europe / North America / South America / Asia / Oceania; an unknown
    id falls back to `europe`. Their default comes from the browser's
    time zone (Europe/Africa → europe, Americas → north-america except
    South American zones → brazil, Asia → east-asia except the Middle
    East, Australia/Pacific → oceania). ARClens used `south-america` and
    `asia` before: those silently got the European schedule. **Verified**
    from their front-end code; not requested live (blocked from this
    environment).
  - Marker data comes straight from their Supabase backend
    (`sb.metaforge.app/rest/v1/arc_map_data`, `map_inspector_data`,
    `custom_map_markers`), not through the public API. Undocumented
    backend: **not to be used** (AGENTS.md rule 2).
  - **Place-name labels** (the Labels/Zones layer) are hard-coded in a
    front-end JavaScript chunk as `{lat, lng, text}` (118 entries across
    maps, e.g. "Pattern House", "Pipeline Tower", region names in
    capitals), in the same coordinate space as the markers. They are not in
    any API.
  - **Maintainer's decision (2026-10-03):** use them. They are the game's
    own place names and positions ("they pulled it from the game, it's not
    theirs"). They are extracted **once, offline**, from a HAR of the map
    page with `scripts/extract-map-labels.py` into
    `crates/arclens-data/data/map-labels.json` (178 labels, 6 maps); the app
    never fetches MetaForge's bundle.
  - Anchor point (**verified** against the game): on layers with
    `center:!0` (Spaceport, Buried City, Blue Gate, Stella Montis) a label's
    position is the text centre; otherwise (Dam, Riven Tides) its top-left
    corner. With the right anchor, OCR'd labels fit within 0–3 px (Dam: 8
    labels at mid zoom; Buried City: 10), and the game's own hatch and
    elevator icons land within ~12 px of MetaForge's markers (2560×1440).
- `game-map-data?tableID=arc_map_data&mapID=dam` returns:

  ```json
  {"allData":[{"id":"uuid","lat":2495.7,"lng":5211.4,"mapID":"dam","category":"arc","subcategory":"queen","instanceName":"Queen","behindLockedDoor":false,"eventConditionMask":1,"added_by":"…","last_edited_by":"…"}]}
  ```

  - `lat`/`lng` are **pixel coordinates of a Leaflet `CRS.Simple` image map**,
    not geographic coordinates. Importing them needs a per-map
    `Transform` into ARClens map space (`arclens_core::Transform`).
  - `eventConditionMask` is a **bit set over the map's condition list**
    (**verified** 2026-10-03 on Dam's 7662 records). MetaForge's map page
    bundle lists each map's conditions as `eventConditions:[{name,mask}]`,
    where `mask` is the bit number, and the numbering differs per map
    (Cold Snap is 9 on Dam, 11 on the Spaceport, 8 on Buried City). On Dam:
    hurricane caches carry 4096 (bit 12, Hurricane), snow piles and
    candleberries 512 (bit 9, Cold Snap), wasp and rocketeer husks 16
    (bit 4, Husk Graveyard), probes 2 (bit 1, Prospecting Probes),
    assessors and combat supplies 16384 (bit 14, Close Scrutiny), baskets
    576 (Cold Snap + Lush Blooms). 5851 records have no mask and 1247 have
    `1` (bit 0, "No Event"); the latter are ordinary markers (lockers,
    raider caches), so ARClens reads absent, 0 and 1 as "every condition".
    The tables are in `arclens_data::metaforge::CONDITIONS`.
  - The bundle also lists a map not in our list yet, `pendola-pass`
    (conditions: No Event only), 2026-10-03.
  - ARClens imports them as `x = lng`, `y = lat`: `lat` grows *down* the
    map (**verified** 2026-10-03 on Dam: Victory Ridge, lat 1461, is at the
    top of the in-game map; Formicai Hills, lat 3915, at the bottom).

## ardb.app

- Closed source, public API at `https://ardb.app/api`:
  `/items`, `/items/{id}`, `/quests`, `/quests/{id}`, `/arc-enemies`,
  `/arc-enemies/{id}`. Images are under `https://ardb.app/static`.
- Requires attribution and a link back. No published licence or rate limit.
- **No map or marker endpoints.**
- Not to be confused with "ARDB" on GitHub (Teyk0o/ARDB = arcraidersdatabase.com,
  CC BY-NC-ND).

## arcraidershub.com

- No public API and no source repo found. Its ToS asks people to contact them
  before extensive reuse. **Do not scrape it.**
- Rechecked 2026-10-03 after the user asked about using its POIs: its map
  pages carry POIs, but there is still no documented API, and the domain
  is blocked from our dev container. Using its marker data would mean
  scraping, which rule 2 forbids. If we want its data, ask the site
  owners for permission first.

## Mahcks/arcraiders-data-api (`arcdata.mahcks.com`)

- Checked 2026-10-03 (repo at commit `d135a8c`, Dec 2025). Open source, a
  Cloudflare Worker.
- It is a **read-through mirror of RaidTheory/arcraiders-data**
  (`raw.githubusercontent.com/RaidTheory/arcraiders-data/main`). It has
  `/v1/items`, `/v1/maps`, `/v1/map-events`, `/v1/quests`, `/v1/hideout`
  and so on.
- It adds no data of its own. We already read RaidTheory directly, so it
  brings nothing new.
- RaidTheory's `maps.json` holds only `id`, localised `name` and an `image`
  URL (`cdn.arctracker.io`): **no POIs or coordinates**. `map-events.json`
  holds the event *types* (name, icon, category, localisations) and a map
  list: **no schedule times**. Verified on a checkout dated 2026-08-18.
- So for markers and the condition schedule, MetaForge is still the only
  documented API we know of.

## Image hosts (2026-10-03)

- ARClens downloads map-condition icons once and caches them
  (`arclens_data::ImageCache`, `~/.cache/arclens/images/`). Only these
  hosts are fetched:
  - `cdn.metaforge.app`: the `icon` of each `events-schedule` entry (often
    `.webp`, sometimes null);
  - `cdn.arctracker.io`: RaidTheory's `map-events.json` icon per condition
    type (`.png`), used when MetaForge gives none.
- Map-marker icons are not downloaded: ARClens draws its own glyphs.

## Where do event timers come from?

- Map conditions (Night Raid, Electromagnetic Storm, Hurricane, Cold Snap,
  Harvester, Matriarch, Husk Graveyard, Prospecting Probes, …) follow a
  **fixed rotation in UTC**.
- Sites hard-code that rotation. MetaForge's `event-timers` table and
  [lpjhelder/arcdb `events.ts`](https://github.com/lpjhelder/arcdb/blob/HEAD/src/data/events.ts)
  are both hand-maintained tables.
- **No Embark feed** for conditions has been found.
- **Unverified:** since patch 1.42 (Aug 2026) regions (EU/NA/SA/Asia/OCE) may
  run the same cycle offset by some hours. Some trackers now show a region
  selector. Verify in-game before shipping timers.

## Where do map markers come from?

- **Crowd-sourced and hand-placed.** MetaForge records carry `added_by` /
  `last_edited_by` usernames. Other community maps such as
  [jakebry/arc-raiders-map](https://github.com/jakebry/arc-raiders-map) (MIT)
  place markers by hand in image pixel space.
- Datamining is *possible*: the UE5 paks open in FModel/CUE4Parse with a
  community `.usmap`. Whether an AES key is needed is unclear. No marker site
  is known to use datamined world coordinates.
- Embark's ToS very likely forbids reverse engineering. We do **not**
  datamine.

## Open-source status

| Project | Source | Licence / terms |
|---|---|---|
| RaidTheory/arcraiders-data | Open | MIT, attribution requested |
| jakebry/arc-raiders-map | Open | MIT |
| arcraiders.wiki | Wiki | CC BY-SA (unverified) |
| Soygen/ARLO | Open | MIT. Python + Tesseract OCR overlay, uses MetaForge |
| MetaForge | Closed | API with attribution, non-commercial without contact |
| ardb.app | Closed | API with attribution |
| arcraidershub.com | Closed | No API |

## Maps (as of 2026-10-03)

- Dam Battlegrounds
- The Spaceport
- Buried City
- The Blue Gate
- Stella Montis (upper / lower)
- Riven Tides (added 2026-04-28)
- **Pendola Pass** is announced for the "Frozen Trail" update on 2026-10-08.
