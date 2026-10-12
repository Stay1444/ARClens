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
status-refresh-failed = Could not refresh game data; showing the saved copy.
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
home-schedule-unavailable = The schedule is unavailable right now (offline?). It is retried every two minutes.
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
items-all-types = All types
items-all-rarities = All
items-show-on-map = Show on map
items-browsing = Browsing { $shown } of { $total } items · type to search
items-results = { $count ->
    [one] { $count } result
   *[other] { $count } results
}
items-pick = Pick an item
items-pick-help = Search above (Enter opens the top result) or browse the list. In game, Ctrl+Shift+I opens the overlay's quick search, where picked items open in their own windows.

## Map

map-unknown = Unknown map
map-loading = Loading markers…
map-focus-title = Where to find { $item }
map-focus-count = { $zones } zones, { $pickups } to pick up, { $containers } containers in its loot areas
map-focus-none = Nothing on this map: try another, or it only drops from ARC and containers without a loot area.
map-focus-clear = Show all markers
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

progress-stash = Stash
progress-stash-help = Open your stash in the game and scroll through it once: ARClens reads every slot and tags it with what to do. Hover an item to confirm it, and ARClens knows it for sure from then on.
stash-summary = { $slots } slots · { $date }
stash-never = Not scanned yet
stash-never-help = No scan yet. With game capture on, open the stash (Inventory tab) and scroll from top to bottom.
stash-latest = Latest scan
stash-scan-line = { $date } · { $slots } slots · { $value } value
stash-likely = { $count } slots recognised from their icon only (marked "?" in game). Hover them once to confirm.
stash-scanning = Scanning: { $seen } of { $used } slots seen. Keep scrolling.
stash-scanning-unknown = Scanning: { $seen } slots seen.
stash-needed = Still needed
stash-needed-help = What your remaining workshop upgrades, quests and projects take, against what your latest scan found.
stash-missing = { $count } short
stash-have-need = { $have } / { $need }
stash-needed-none = Nothing left to collect.
stash-history = History
stash-no-changes = No changes since the scan before.
stash-more = , and { $count } more

## Settings

settings-title = Settings
settings-language = Language
settings-language-help = The language of the app and the overlay. Item, quest and station names come from the game data in this language too.
settings-language-system = System ({ $name })
settings-overlay-size = Overlay size
settings-overlay-size-help = Scales everything the overlay draws: cards, the map panel, markers and the quick search.
settings-pinned-card = Status badge
settings-pinned-card-help = The corner of the screen where the overlay shows its status while it is visible.
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

## The overlay's item window (built by the app)

detail-crafting = Crafting
detail-upgrades = Upgrades
detail-repair = Repair
detail-repair-restores = Repair (restores { $percent } %)
detail-sold-by = Sold by
detail-unlocks = Unlocks crafting
detail-mod-slots = Mod slots
detail-fits = Fits
detail-weight = Weight
detail-value-per-kg = Value per kg
detail-stack = Stack size
detail-found-in = Found in
detail-value-per-slot = Value per slot
detail-where-to-look = Where to look
detail-loot-high = high loot
detail-loot-medium = medium loot
detail-dropped-by = Dropped by
detail-source = Data from
detail-source-note = { $source } (newer than the main dataset; may be incomplete)
detail-blueprint = Blueprint
detail-blueprint-learned = Learned
detail-blueprint-missing = Not learned
detail-blueprint-needed = Needed
detail-quest-item = Quest item
detail-yes = Yes
detail-added-in = Added in
detail-base-game = Launch
detail-in-raid = In raid
detail-level = level { $level }
detail-makes = makes { $count }
detail-upgrade-from = upgrades from
detail-upgrade-to = upgrades to
detail-per-day = { $count } a day
detail-limit = { $count } per restock
detail-currency-coins = coins
detail-currency-creds = creds
detail-slot-muzzle = Muzzle
detail-slot-grip = Grip
detail-slot-stock = Stock
detail-slot-magazine = Magazine
detail-slot-special = Special

## Start-up screen

startup-tagline = Item advice, maps and timers for ARC Raiders
startup-skip = Skip
startup-data = Game data
startup-items = { $count } items
startup-unavailable = unavailable, will retry
startup-schedule = Condition schedule
startup-conditions = { $count } conditions
startup-overlay = Overlay
startup-capture = Game capture
startup-off = off
startup-waiting-game = waiting for the game

## Condition alerts

events-alert-on = Alerting
events-alert-off = Alert me
events-alert-hint = Alert me sends a desktop notification and an in-game notice 5 minutes before a condition starts.
alert-kicker = Condition alert
alert-title = { $condition } in { $minutes } min
alert-body = { $map } · { $start } – { $end }
alert-open = Open ARClens

## Stash: make room

stash-room = Make room
stash-room-help = From the latest scan: what can go to free stash slots, most slots first. Sell and recycle advice, plus spare copies of items you keep beyond what is still needed.
stash-room-slots = { $slots } slots
stash-room-line = ×{ $count } · { $slots } { $slots -> 
    [one] slot
   *[other] slots
} · { $how }
stash-room-spare = spares
stash-room-recycle = recycle
stash-room-sell = sell

## Blueprint hunt

hunt-unlocks = Unlocks { $items }
hunt-found-in = drops in { $places }
hunt-dropped-by = dropped by { $arc }
hunt-open = Open
hunt-help = Not learned yet first. Blueprints come out of containers; the game marks these zones as high loot: { $zones }.

## Progress → Planner

progress-planner = Planner
progress-planner-help = Pick any goals, all at once and in no order: benches to upgrade, projects and expeditions to deliver, items to craft. ARClens adds up what they take, counts your stash once, and shows what to craft from parts and what is missing.
planner-goal-craft = Craft { $item }
planner-stash-from = Stash from the scan of { $date }.
planner-no-scan = No stash scan yet: everything counts as missing. Open your stash in game and scroll through it once.
planner-materials = Materials
planner-ready = All there
planner-missing = { $count } to find
planner-in-stash = { $count } in stash
planner-craft = craft { $count }
planner-to-find = { $count } missing
planner-recycle = Recycle for parts
planner-recycle-help = Spare items in your stash (ones advised to sell or recycle, or kept beyond what is needed) that break down into missing parts.
planner-nothing = Nothing to plan: this goal takes no items.
planner-summary-ready = ready
planner-summary-missing = { $count } missing
planner-summary-none = no goal
items-plan-craft = Plan crafting
planner-reach = How far
planner-reach-next = Next level / phase
planner-reach-max = All the way
planner-all-benches = All benches
planner-station-chip = { $station } → { $level }
planner-project-chip = { $project } → { $phase }/{ $total }
planner-workshop = Workshop
planner-projects = Projects and expeditions
planner-crafting = Crafting
planner-crafting-help = Add items from their page with "Plan crafting".
planner-clear = Clear goals
planner-goal-count = { $count ->
    [one] 1 goal
   *[other] { $count } goals
}
