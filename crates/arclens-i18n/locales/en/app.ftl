# The companion app.

## Top bar

tab-home = Home
tab-items = Items
tab-map = Map
tab-events = Events
tab-progress = Progress
tab-settings = Settings
top-overlay-offline = Overlay offline
top-overlay-shown = Overlay shown
top-overlay-hidden = Overlay hidden
top-capture = Capture
top-game-capture = Game capture
top-capture-auto-running = auto · game running
top-capture-auto-waiting = auto · waiting for game
top-capture-always = always
top-capture-off = off
top-show-overlay = Show overlay
top-hide-overlay = Hide overlay
top-show = Show
top-hide = Hide
top-overlay = overlay
top-interactive = Interactive
top-click-through = Click-through
top-mode = mode

## Status line

status-no-hotkeys = Global hotkeys unavailable (no GlobalShortcuts portal); use the buttons above.
status-overlay-not-started = Overlay not started: { $error }
status-overlay-link-failed = Overlay link failed: { $error }
status-overlay-outdated = Overlay is out of date ({ $error }): rebuild it with `cargo build --release -p arclens-overlay`
status-detection-off = Item detection off: { $reason }
# Ends every "read from the game" note, so a newer one replaces it.
status-from-game-marker = (read from the game)
status-level-from-game = { $station } set to level { $level } (read from the game).
status-quests-from-game = Quest progress updated (read from the game).
status-refreshing = Refreshing game data…
error-schedule = could not load the event schedule: { $error }
error-markers = could not load markers: { $error }
error-overlay-start = could not start { $bin }: { $error }
status-ocr-unavailable = OCR model unavailable: { $error }
status-capture-unavailable = screen capture unavailable: { $error }

## Home

home-tagline = Companion and overlay for ARC Raiders
home-search = Look up an item…
home-search-count = Look up any of { $count } items…
home-status-game = Game
home-status-running = Running
home-status-not-running = Not running
home-status-capture = Game capture
home-status-capture-on = On (auto)
home-status-capture-waits = Waits for game
home-status-capture-always = Always on
home-status-off = Off
home-status-overlay = Overlay
home-status-offline = Offline
home-status-shown = Shown
home-status-ready = Ready
home-conditions = Map conditions
home-all-events = All events →
home-loading-schedule = Loading the schedule…
home-now = Now
home-next = Next
home-in = In { $time }
home-nothing-running = Nothing running right now.
home-maps = Maps
home-last-seen = Last seen in game
home-preset = Preset: { $name }
home-open-map-hint = Open the map in game and markers appear on it.
home-progress = Progress
home-levels-built = / { $total } levels built
home-levels-help = Advice keeps what your next upgrades need.
home-levels-unset = Set your workshop levels so keep / sell / recycle advice knows what you still need.
home-edit-progress = Edit progress
home-in-game = In game
home-tip-hover = Hover an item
home-tip-hover-body = A card beside the game's tooltip says keep, sell or recycle.
home-tip-map = Open the map
home-tip-map-body = Markers follow the map; the panel bottom right switches presets.
home-tip-toggle-body = Show or hide the overlay.

## Items

data-loading = Loading game data…
data-load-failed = Could not load game data
items-search = Search items…
items-browsing = Browsing { $shown } of { $total } items · type to search
items-results = { $count ->
    [one] { $count } result
   *[other] { $count } results
}
items-pick = Pick an item
items-pick-help = Search above (Enter opens the top result) or browse the list. The selected item is also shown on the in-game overlay.

## Map

map-unknown = Unknown map
map-loading = Loading markers…
map-load-failed = Could not load markers
map-condition = Condition
map-condition-any = Any
map-floor = Floor
map-floor-upper = Upper
map-floor-lower = Lower
map-shown = { $shown } / { $total } shown
map-show-all = Show all
map-hide-all = Hide all
map-preset = Preset
map-search = Search markers…
map-markers = Markers
map-matches = Matches
map-preset-save-to = Save to "{ $name }"
map-preset-reset = Reset to default
map-preset-delete = Delete
map-preset-new-name = New preset name…
map-preset-save-new = Save as new
map-preset-this-map = This map only
map-preset-condition-only = { $condition } only
marker-locked = locked

## Built-in map presets

preset-everything = Everything
preset-everything-desc = Every marker the map has.
preset-loot-run = Loot run
preset-loot-run-desc = Worthwhile containers, field crates and ways out; no common boxes.
preset-ways-out = Ways out
preset-ways-out-desc = Extractions, raider hatches, supply stations and key rooms.
preset-arc-threats = ARC threats
preset-arc-threats-desc = Where ARC machines patrol or sit.
preset-quests = Quests
preset-quests-desc = Quest objectives, plus the ways out.
preset-gathering = Gathering
preset-gathering-desc = Plants, fruit and baskets.
preset-first-wave-caches = First Wave caches
preset-first-wave-caches-desc = Hurricane: First Wave caches (rare blueprints) and raider caches, which share spawn spots, plus the ways out.
preset-uncovered-caches = Uncovered caches
preset-uncovered-caches-desc = Raider caches, plus the ways out.
preset-cold-snap = Snow piles
preset-cold-snap-desc = Cold Snap: snow piles and candleberries, plus the ways out.
preset-husk-graveyard = Husks
preset-husk-graveyard-desc = Husk Graveyard: ARC husks to loot, plus the ways out.
preset-probes = Probes and couriers
preset-probes-desc = ARC probes and couriers, plus the ways out.
preset-lush-blooms = Blooms
preset-lush-blooms-desc = Lush Blooms: plants and baskets, plus the ways out.
preset-close-scrutiny = Assessors
preset-close-scrutiny-desc = Close Scrutiny: assessors, combat supplies and vaporizers, plus the ways out.
preset-harvester = Harvester
preset-harvester-desc = The Harvester and the Queen, plus the ways out.
preset-matriarch = Matriarch
preset-matriarch-desc = The Matriarch, plus the ways out.

## Events

events-title = Events
events-loading = Loading event schedule…
events-load-failed = Could not load the event schedule
events-active-now = Active now
events-next-per-map = Next on each map
events-schedule = Schedule
events-nothing = Nothing here right now.
events-all-maps = All maps
events-ends-in = Ends in { $time }
events-ends-in-short = ends in { $time }
events-starts-in = Starts in { $time }
events-footnote = Times in your local time zone. Some sites shift the rotation per server region; if times look off for you, tell us.
events-pick-region = Which servers do you play on? Condition times differ by region; pick yours above. Showing { $showing } for now.
events-default-schedule = the default schedule
events-region-schedule = { $region }
events-other-region = MetaForge returned the { $served } schedule, not { $chosen }: times may be off.
weekday-mon = Mon
weekday-tue = Tue
weekday-wed = Wed
weekday-thu = Thu
weekday-fri = Fri
weekday-sat = Sat
weekday-sun = Sun

## Progress

progress-workshop = Workshop
progress-quests = Quests
progress-projects = Projects
progress-blueprints = Blueprints
progress-levels-summary = { $done } of { $total } levels
progress-quests-summary = { $done } of { $total } done
progress-phases-summary = { $done } of { $total } phases
progress-blueprints-summary = { $done } of { $total } learned
progress-autofill = Fills in from the game
progress-autofill-help = With game capture on, open a station in the game's Workshop: ARClens reads its level from the page title and updates it here.
progress-levels-used = Advice uses these levels.
progress-levels-unset = Not set yet: advice is based on value only.
progress-workshop-help = Your workshop levels tell advice what you still need: upgrades you've built stop counting as reasons to keep an item, and parts for the next ones make recycling worth it.
progress-stations = Stations
progress-forget = Forget my progress
progress-needs-items = needs items
progress-other-trader = Other
progress-quests-help = Tick the quests you've finished: items they asked for stop counting as reasons to keep. Ticking a quest also ticks the ones before it.
progress-all-phases-done = All phases done
progress-next-phase = Next: { $phase }
progress-projects-help = Set how many phases of each project you've delivered: items for finished phases stop counting as reasons to keep.
progress-blueprints-help = Blueprints you haven't learned show LEARN instead of a price. Tick the ones you know: a duplicate is then just worth its price.

## Settings

settings-title = Settings
settings-language = Language
settings-language-help = The language of the app and the overlay. Item, quest and station names come from the game data in this language too.
settings-language-system = System ({ $name })
settings-overlay-size = Overlay size
settings-overlay-size-help = Scales everything the overlay draws: cards, the map panel, markers and the quick search.
settings-pinned-card = Pinned item card
settings-pinned-card-help = Where the card of the item you pick (in the app or the overlay's quick search) sits while the overlay is shown.
settings-overlay-background = Overlay background
settings-overlay-background-help = How much of the game shows through the overlay's cards and panels.
settings-opacity-solid = Solid
settings-game-data = Game data
settings-game-data-help = How often item data and map markers are fetched again. Data changes with game patches; the cache keeps working offline.
settings-refresh-now = Refresh now
settings-refresh-hours = { $count } hours
settings-refresh-days = { $count } days
settings-refresh-daily = Daily
settings-refresh-weekly = Weekly
settings-region = Server region
settings-region-help = Map condition times differ per region.
settings-about = ARClens { $version } · GPL-3.0-or-later · github.com/Stay1444/ARClens
corner-top-left = Top left
corner-top-right = Top right
corner-bottom-left = Bottom left
corner-bottom-right = Bottom right
