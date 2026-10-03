#!/usr/bin/env python3
"""Extract the in-game map place names (labels) from a HAR of MetaForge's
map page into data/map-labels.json.

The names and positions are the game's own map labels; MetaForge ships them
in its front-end bundle, in the same coordinate space as its marker API. We
take them once, offline, from a HAR the maintainer recorded (decision
2026-10-03, see docs/research/data-sources.md) and commit the result; the
app never fetches the bundle.

Usage: scripts/extract-map-labels.py metaforge.app.har \
         > crates/arclens-data/data/map-labels.json
"""

import json
import re
import sys

# A label object: `{lat:…,lng:…,text:"…",zlayers:…}`, fields in any order.
OBJECT = re.compile(r"\{([^{}]*)\}")
FIELD = re.compile(r'(\w+):("[^"]*"|-?[\d.e]+)')
LAYER = re.compile(r'key:"(?P<key>\w+)",displayName:"(?P<name>[^"]+)"')


def bundle_with_labels(har):
    for entry in har["log"]["entries"]:
        text = entry["response"]["content"].get("text") or ""
        if "labelCategories:" in text and "MapConfigs" in text:
            return text
    sys.exit("no MetaForge map bundle with labelCategories in this HAR")


def main():
    har = json.load(open(sys.argv[1], encoding="utf-8"))
    js = bundle_with_labels(har)

    # Each map config names its label variable: {id:"dam",…,labelCategories:f,…}
    configs = re.findall(r'\{id:"([\w-]+)",tableID:[^{}]*?.*?labelCategories:(\w+)', js)
    # Where each label variable is defined: `,f=[{`
    starts = {}
    for _, var in configs:
        m = re.search(r'[,\s]' + re.escape(var) + r'=\[\{key:"', js)
        if m:
            starts[var] = m.start()
    bounds = sorted(starts.values()) + [js.index("MapConfigs")]

    out = {}
    for map_id, var in configs:
        if var not in starts:
            continue
        begin = starts[var]
        end = min(b for b in bounds if b > begin)
        chunk = js[begin:end]
        labels = []
        # Walk layers in order; each label belongs to the layer before it.
        # A layer with `center:!0` anchors its labels at the text centre;
        # without it, at the text's top-left corner (verified on Dam against
        # the game: 0-2 px).
        layer_marks = []
        for m in LAYER.finditer(chunk):
            head = chunk[m.end() : chunk.find("labels:[", m.end())]
            layer_marks.append((m.start(), m["key"], "center:!0" in head))
        for m in OBJECT.finditer(chunk):
            fields = {k: v.strip('"') for k, v in FIELD.findall(m.group(1))}
            if not {"text", "lat", "lng"} <= fields.keys():
                continue  # zone polygon vertex, style, …
            layer, centered = next(
                ((k, c) for pos, k, c in reversed(layer_marks) if pos < m.start()),
                (None, False),
            )
            label = {
                "text": fields["text"],
                "lat": float(fields["lat"]),
                "lng": float(fields["lng"]),
                "layer": layer,
                "anchor": "center" if centered else "top-left",
            }
            if "zlayers" in fields:
                label["zlayers"] = int(fields["zlayers"])
            labels.append(label)
        out[map_id] = labels

    json.dump(
        {
            "_source": "Game map labels as shipped by metaforge.app's map page "
            "(extracted with scripts/extract-map-labels.py)",
            "maps": out,
        },
        sys.stdout,
        indent=1,
        ensure_ascii=False,
    )
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
