# Vision fixtures

Frames from the maintainer's own ARC Raiders screen recordings
(2026-10-03, 2560×1440, KDE Plasma 6, Proton). They are committed with the
maintainer's permission, for testing only. Game UI © Embark Studios AB.

| Prefix | Source | Notes |
|---|---|---|
| `stash_*` | Main-menu stash / loadout | Tooltip shows the sell value in its footer |
| `raid_*` | Backpack during a raid | Header tab reads "PING ITEM" / "REQUEST …"; **no sell value** in the footer |
| `trader_*` | Trader (Tian Wen) | A persistent cream purchase panel is always present; hover tooltips can touch or overlap it |
| `map/dam_*` | In-raid map screen, Dam Battlegrounds | Zoomed out / mid / in, and two POI hover cards |
| `*_none*` | Same screens, nothing hovered | Must produce no tooltip |
| `workshop_overview` | Workshop tab, station tiles with roman-numeral levels | Screenshot 2026-10-04 |
| `quest_*` | A trader's QUESTS tab, one quest selected | Screenshots 2026-10-04 (Apollo, Lance) |
| `logbook` | Pause menu, LOGBOOK tab: active quests, tracked resources | Screenshot 2026-10-04; the list scrolls |
| `projects_overview`, `project_*` | Projects list and project pages (phase circles) | Screenshots 2026-10-04 |
| `stash_heavy_ammo`, `stash_looting_mk2` | Pause menu stash, from a 2026-10-05 recording | A short ammo tooltip; a chip glyph with a dark speck |
| `inventory_stash` | Pause menu, INVENTORY tab, nothing hovered | Screenshot 2026-10-04 |

Frames are JPEG (quality ≈ 90), extracted at 1 fps with ffmpeg or
converted from the maintainer's PNG screenshots. Don't add
full-resolution PNGs: crops or JPEGs only, to keep the repo small.
