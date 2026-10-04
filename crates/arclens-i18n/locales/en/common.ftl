# Shared by the app and the overlay: the item card and game names.

## Verdicts (the card's big word)

verdict-keep = KEEP
verdict-sell = SELL
verdict-recycle = RECYCLE
verdict-salvage = SALVAGE
verdict-learn = LEARN
verdict-unknown = NO DATA

## Why that verdict, one line

reason-worth-keeping = Worth keeping
reason-needed-for = Needed for { $name }{ $more ->
    [0] {""}
   *[other] {" "}+{ $more } more
}
reason-parts-needed = Parts needed for { $upgrade }
reason-more-than-selling = +{ $amount } more than selling
reason-worth-more-as-parts = Worth more as parts
reason-more-than-recycling = +{ $amount } more than recycling
reason-more-than-salvaging = Carry out: +{ $amount } more than salvaging
reason-same-value = Same value either way
reason-no-useful-parts = Doesn't recycle into anything useful
reason-learn = Learn it to unlock crafting rather than selling it
reason-no-data = No value data for this item

## Item card

card-sell = Sell
card-sell-base = Sell (base value)
card-recycle = Recycle
card-salvage = Salvage
card-recycles-into = Recycles into
card-salvages-into = Salvages into
card-needed-for = Needed for
card-used-to-craft = Used to craft
card-parts-would-help = Parts would help with
card-parts-would-help-line = { $upgrade } (recycle if you're short on parts)
card-more = +{ $count } more
item-count = { $count ->
    [one] { $count } item
   *[other] { $count } items
}

kind-quest = Quest
kind-workshop = Workshop
kind-project = Project
kind-crafting = Crafting

## Rarities

rarity-common = Common
rarity-uncommon = Uncommon
rarity-rare = Rare
rarity-epic = Epic
rarity-legendary = Legendary
