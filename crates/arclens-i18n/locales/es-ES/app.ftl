# La app de escritorio.

## Barra superior

tab-home = Inicio
tab-items = Objetos
tab-map = Mapa
tab-events = Eventos
tab-progress = Progreso
tab-settings = Ajustes
top-overlay-offline = Overlay desconectado
top-overlay-shown = Overlay visible
top-overlay-hidden = Overlay oculto
top-capture = Captura
top-game-capture = Captura del juego
top-capture-auto-running = auto · juego abierto
top-capture-auto-waiting = auto · esperando al juego
top-capture-always = siempre
top-capture-off = desactivada
top-show-overlay = Mostrar overlay
top-hide-overlay = Ocultar overlay
top-show = Mostrar
top-hide = Ocultar
top-overlay = overlay
top-interactive = Interactivo
top-click-through = Clic a través
top-mode = modo

## Línea de estado

status-no-hotkeys = Atajos globales no disponibles (sin portal GlobalShortcuts); usa los botones de arriba.
status-overlay-not-started = El overlay no se ha iniciado: { $error }
status-overlay-link-failed = Falló la conexión con el overlay: { $error }
status-overlay-outdated = El overlay está desactualizado ({ $error }): vuelve a compilarlo con `cargo build --release -p arclens-overlay`
status-detection-off = Detección de objetos desactivada: { $reason }
# Termina cada nota «leído del juego», para que una nueva sustituya a la anterior.
status-from-game-marker = (leído del juego)
status-level-from-game = { $station } pasa al nivel { $level } (leído del juego).
status-quests-from-game = Progreso de misiones actualizado (leído del juego).
status-refreshing = Actualizando los datos del juego…
error-schedule = no se ha podido cargar el calendario de eventos: { $error }
error-markers = no se han podido cargar los marcadores: { $error }
error-overlay-start = no se ha podido iniciar { $bin }: { $error }
status-ocr-unavailable = modelo de OCR no disponible: { $error }
status-capture-unavailable = captura de pantalla no disponible: { $error }

## Inicio

home-tagline = Compañero y overlay para ARC Raiders
home-search = Busca un objeto…
home-search-count = Busca entre { $count } objetos…
home-status-game = Juego
home-status-running = Abierto
home-status-not-running = Cerrado
home-status-capture = Captura del juego
home-status-capture-on = Activa (auto)
home-status-capture-waits = Espera al juego
home-status-capture-always = Siempre activa
home-status-off = Desactivada
home-status-overlay = Overlay
home-status-offline = Desconectado
home-status-shown = Visible
home-status-ready = Listo
home-conditions = Condiciones de mapa
home-all-events = Todos los eventos →
home-loading-schedule = Cargando el calendario…
home-now = Ahora
home-next = Próximas
home-in = En { $time }
home-nothing-running = No hay nada activo ahora mismo.
home-maps = Mapas
home-last-seen = Visto por última vez en el juego
home-preset = Preajuste: { $name }
home-open-map-hint = Abre el mapa en el juego y aparecerán los marcadores encima.
home-progress = Progreso
home-levels-built = / { $total } niveles construidos
home-levels-help = Los consejos guardan lo que piden tus próximas mejoras.
home-levels-unset = Indica los niveles de tu taller para que los consejos de guardar / vender / reciclar sepan qué te falta.
home-edit-progress = Editar progreso
home-in-game = En el juego
home-tip-hover = Pasa el ratón por un objeto
home-tip-hover-body = Una tarjeta junto al tooltip del juego te dice si guardarlo, venderlo o reciclarlo.
home-tip-map = Abre el mapa
home-tip-map-body = Los marcadores siguen al mapa; el panel de abajo a la derecha cambia de preajuste.
home-tip-toggle-body = Muestra u oculta el overlay.

## Objetos

data-loading = Cargando los datos del juego…
data-load-failed = No se han podido cargar los datos del juego
items-search = Buscar objetos…
items-browsing = { $shown } de { $total } objetos · escribe para buscar
items-results = { $count ->
    [one] { $count } resultado
   *[other] { $count } resultados
}
items-pick = Elige un objeto
items-pick-help = Busca arriba (Intro abre el primer resultado) o recorre la lista. El objeto elegido también se muestra en el overlay del juego.

## Mapa

map-unknown = Mapa desconocido
map-loading = Cargando marcadores…
map-load-failed = No se han podido cargar los marcadores
map-condition = Condición
map-condition-any = Cualquiera
map-floor = Planta
map-floor-upper = Superior
map-floor-lower = Inferior
map-shown = { $shown } / { $total } visibles
map-show-all = Mostrar todo
map-hide-all = Ocultar todo
map-preset = Preajuste
map-search = Buscar marcadores…
map-markers = Marcadores
map-matches = Coincidencias
map-preset-save-to = Guardar en «{ $name }»
map-preset-reset = Restablecer
map-preset-delete = Eliminar
map-preset-new-name = Nuevo preajuste…
map-preset-save-new = Guardar como nuevo
map-preset-this-map = Solo este mapa
map-preset-condition-only = Solo con { $condition }
marker-locked = cerrado

## Preajustes de mapa incluidos

preset-everything = Todo
preset-everything-desc = Todos los marcadores del mapa.
preset-loot-run = Ruta de botín
preset-loot-run-desc = Contenedores que merecen la pena, cajas de campo y salidas; sin cajas comunes.
preset-ways-out = Salidas
preset-ways-out-desc = Extracciones, escotillas de asaltante, estaciones de suministros y salas con llave.
preset-arc-threats = Amenazas ARC
preset-arc-threats-desc = Dónde patrullan o esperan las máquinas ARC.
preset-quests = Misiones
preset-quests-desc = Objetivos de misión, y las salidas.
preset-gathering = Recolección
preset-gathering-desc = Plantas, fruta y cestas.
preset-first-wave-caches = Alijos de la Primera Ola
preset-first-wave-caches-desc = Huracán: alijos de la Primera Ola (planos raros) y alijos de asaltante, que comparten puntos de aparición, y las salidas.
preset-uncovered-caches = Alijos al descubierto
preset-uncovered-caches-desc = Alijos de asaltante, y las salidas.
preset-cold-snap = Montones de nieve
preset-cold-snap-desc = Ola de frío: montones de nieve y bayas de cera, y las salidas.
preset-husk-graveyard = Carcasas
preset-husk-graveyard-desc = Cementerio de carcasas: carcasas ARC que saquear, y las salidas.
preset-probes = Sondas y mensajeros
preset-probes-desc = Sondas y mensajeros ARC, y las salidas.
preset-lush-blooms = Floración
preset-lush-blooms-desc = Floración exuberante: plantas y cestas, y las salidas.
preset-close-scrutiny = Evaluadores
preset-close-scrutiny-desc = Vigilancia estrecha: evaluadores, suministros de combate y vaporizadores, y las salidas.
preset-harvester = Cosechadora
preset-harvester-desc = La Cosechadora y la Reina, y las salidas.
preset-matriarch = Matriarca
preset-matriarch-desc = La Matriarca, y las salidas.

## Eventos

events-title = Eventos
events-loading = Cargando el calendario de eventos…
events-load-failed = No se ha podido cargar el calendario de eventos
events-active-now = Activos ahora
events-next-per-map = Próximo en cada mapa
events-schedule = Calendario
events-nothing = No hay nada aquí ahora mismo.
events-all-maps = Todos los mapas
events-ends-in = Termina en { $time }
events-ends-in-short = termina en { $time }
events-starts-in = Empieza en { $time }
events-footnote = Horas en tu zona horaria. Algunas webs desplazan la rotación según la región del servidor; si las horas no te cuadran, avísanos.
events-pick-region = ¿En qué servidores juegas? Las horas de las condiciones cambian según la región; elige la tuya arriba. De momento se muestra { $showing }.
events-default-schedule = el calendario por defecto
events-region-schedule = el de { $region }
events-other-region = MetaForge ha devuelto el calendario de { $served }, no el de { $chosen }: las horas pueden no cuadrar.
weekday-mon = lun
weekday-tue = mar
weekday-wed = mié
weekday-thu = jue
weekday-fri = vie
weekday-sat = sáb
weekday-sun = dom

## Progreso

progress-workshop = Taller
progress-quests = Misiones
progress-projects = Proyectos
progress-blueprints = Planos
progress-levels-summary = { $done } de { $total } niveles
progress-quests-summary = { $done } de { $total } hechas
progress-phases-summary = { $done } de { $total } fases
progress-blueprints-summary = { $done } de { $total } aprendidos
progress-autofill = Se rellena desde el juego
progress-autofill-help = Con la captura del juego activa, abre una estación en el Taller del juego: ARClens lee su nivel del título de la página y lo actualiza aquí.
progress-levels-used = Los consejos usan estos niveles.
progress-levels-unset = Aún sin indicar: los consejos solo tienen en cuenta el valor.
progress-workshop-help = Los niveles de tu taller le dicen a los consejos qué te falta: las mejoras que ya has construido dejan de ser motivo para guardar un objeto, y las piezas para las siguientes hacen que merezca la pena reciclar.
progress-stations = Estaciones
progress-forget = Olvidar mi progreso
progress-needs-items = pide objetos
progress-other-trader = Otros
progress-quests-help = Marca las misiones que has terminado: los objetos que pedían dejan de ser motivo para guardar. Marcar una misión también marca las anteriores.
progress-all-phases-done = Todas las fases hechas
progress-next-phase = Siguiente: { $phase }
progress-projects-help = Indica cuántas fases de cada proyecto has entregado: los objetos de las fases terminadas dejan de ser motivo para guardar.
progress-blueprints-help = Los planos que no has aprendido muestran APRENDER en vez de un precio. Marca los que ya sabes: un duplicado solo vale su precio.

## Ajustes

settings-title = Ajustes
settings-language = Idioma
settings-language-help = El idioma de la app y del overlay. Los nombres de objetos, misiones y estaciones también salen de los datos del juego en este idioma.
settings-language-system = Sistema ({ $name })
settings-overlay-size = Tamaño del overlay
settings-overlay-size-help = Escala todo lo que dibuja el overlay: tarjetas, el panel del mapa, los marcadores y la búsqueda rápida.
settings-pinned-card = Tarjeta fijada
settings-pinned-card-help = Dónde se coloca la tarjeta del objeto que eliges (en la app o en la búsqueda rápida del overlay) mientras el overlay está visible.
settings-overlay-background = Fondo del overlay
settings-overlay-background-help = Cuánto del juego se ve a través de las tarjetas y paneles del overlay.
settings-opacity-solid = Opaco
settings-game-data = Datos del juego
settings-game-data-help = Cada cuánto se vuelven a descargar los datos de objetos y los marcadores del mapa. Los datos cambian con los parches; la caché sigue funcionando sin conexión.
settings-refresh-now = Actualizar ahora
settings-refresh-hours = { $count } horas
settings-refresh-days = { $count } días
settings-refresh-daily = Cada día
settings-refresh-weekly = Cada semana
settings-region = Región del servidor
settings-region-help = Las horas de las condiciones de mapa cambian según la región.
settings-about = ARClens { $version } · GPL-3.0 o posterior · github.com/Stay1444/ARClens
corner-top-left = Arriba a la izquierda
corner-top-right = Arriba a la derecha
corner-bottom-left = Abajo a la izquierda
corner-bottom-right = Abajo a la derecha
