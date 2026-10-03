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

Frames are JPEG (quality ≈ 90) extracted at 1 fps with ffmpeg. Don't add
full-resolution PNGs: crops or JPEGs only, to keep the repo small.
