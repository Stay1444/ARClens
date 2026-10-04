# Compartido por la app y el overlay: la tarjeta de objeto y los nombres del juego.

## Veredictos (la palabra grande de la tarjeta)

verdict-keep = GUARDAR
verdict-sell = VENDER
verdict-recycle = RECICLAR
verdict-salvage = DESGUAZAR
verdict-learn = APRENDER
verdict-unknown = SIN DATOS

## Por qué, en una línea

reason-worth-keeping = Merece la pena guardarlo
reason-needed-for = Necesario para { $name }{ $more ->
    [0] {""}
   *[other] {" "}y { $more } más
}
reason-parts-needed = Sus piezas sirven para { $upgrade }
reason-more-than-selling = +{ $amount } más que vendiéndolo
reason-worth-more-as-parts = Vale más en piezas
reason-more-than-recycling = +{ $amount } más que reciclándolo
reason-more-than-salvaging = Sácalo: +{ $amount } más que desguazándolo
reason-same-value = Vale lo mismo de las dos formas
reason-no-useful-parts = No da piezas útiles al reciclarlo
reason-learn = Apréndelo para desbloquear su fabricación en vez de venderlo
reason-no-data = No hay datos de valor para este objeto

## Tarjeta de objeto

card-sell = Venta
card-sell-base = Venta (valor base)
card-recycle = Reciclaje
card-salvage = Desguace
card-recycles-into = Al reciclarlo da
card-salvages-into = Al desguazarlo da
card-needed-for = Necesario para
card-used-to-craft = Sirve para fabricar
card-parts-would-help = Sus piezas ayudarían con
card-parts-would-help-line = { $upgrade } (recíclalo si te faltan piezas)
card-more = y { $count } más
item-count = { $count ->
    [one] { $count } objeto
   *[other] { $count } objetos
}

kind-quest = Misión
kind-workshop = Taller
kind-project = Proyecto
kind-crafting = Fabricación

## Rarezas

rarity-common = Común
rarity-uncommon = Poco común
rarity-rare = Rara
rarity-epic = Épica
rarity-legendary = Legendaria
