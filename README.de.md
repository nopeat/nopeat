<div align="center">

<img src="assets/logo.svg" alt="Nopeat logo" width="72" />

# Nopeat

**Ein Analysator für alle Bundler.** Den Abhängigkeitsgraphen aus `stats.json`, die
echte Byte-Zuordnung aus Source Maps, esbuild-Metafiles oder einfach einem
`dist/`-Ordner — zu einem Graphen zusammengeführt, mit einem Bruchteil des
Speichers.

[![CI](https://github.com/Nopeat/Nopeat/actions/workflows/ci.yml/badge.svg)](https://github.com/Nopeat/Nopeat/actions/workflows/ci.yml)
[![status](https://img.shields.io/badge/status-unreleased-orange.svg)](#installation)
[![MSRV](https://img.shields.io/badge/rust-1.90%2B-blue.svg)](https://doc.rust-lang.org/stable/notes.html)
[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

[English](README.en.md) · [中文](README.zh.md) · [日本語](README.ja.md) · **Deutsch**

</div>

<p align="center">
  <a href="#das-problem">das Problem</a> ·
  <a href="#messwerte">Messwerte</a> ·
  <a href="#ghost-code-und-versteckter-code">Ghost &amp; versteckter Code</a> ·
  <a href="#installation">Installation</a> ·
  <a href="#stand">Ehrlicher Stand</a> ·
  <a href="#warum">Warum so gebaut</a>
</p>

---

## Why it is fast

Three decisions, each with a number behind it. The full table is in
[`CHANGELOG.md`](CHANGELOG.md); the measurements are reproducible from
[`bench/results/`](bench/results/).

**It streams instead of building a parse tree.** The obvious implementation
deserialises `stats.json` into a `serde_json::Value` and then walks it. That value
is a tree of boxed maps and owned strings, and on the 1 GB fixture it cost about
550 MB before any analysis started. Modules enter the graph one at a time instead,
so the document is never resident. Peak stays at 376 MB against a 400 MB ceiling.

**It builds an index instead of scanning.** Matching each module to its source was
O(modules × sources): 2.5 billion comparisons, 73.8 seconds on the fused
benchmark. A suffix index built once per asset turns the lookup into a hash, and
the same benchmark drops to 4.94 seconds. A property test asserts the index and the
original scan agree on every awkward case, so it is the same rule with a lookup
table rather than a second implementation.

**It shares what repeats.** A large build has hundreds of thousands of modules and
a few hundred packages. Package references are interned, so every module in a
package points at one allocation. On the 1 GB fixture that was worth about 82 MB.

There is no `unsafe` in any of it — `#![forbid(unsafe_code)]` is enforced by the
compiler, so there is no hand-written SIMD and no unchecked indexing. The
bottleneck was never arithmetic. It was allocation and IO.

## It finds code nothing else can account for

The join between a bundler's module graph and a source map is the part no existing
tool does. `webpack-bundle-analyzer` reads a graph, `source-map-explorer` reads a
map, and neither reconciles them.

Nopeat does, and whatever fails to reconcile is reported rather than hidden:

- **Ghost code** — declared by the bundler, attributed to no source file. Code that
  will ship to production and that no one on the team has traced back.
- **Hidden code** — bytes in the bundle that map back to no module. Minifier
  output, injected polyfills, the bundler's own runtime.

Both show up in the report as first-class rows, not as a footnote. Ghost code
needs a declared graph to be defined against, so with a bare build folder it is
reported as undetectable (`NPT0051`) instead of as a reassuring zero.

## Unterstützte Bundler

| Bundler | Was Nopeat liest | Braucht `dist/` + `*.map`? | Ghost-Code-Erkennung |
|---|---|---|---|
| **webpack** 4 / 5 | `stats.json` | nein | ja |
| **rspack** | `stats.json` (gleiches Schema) | nein | ja |
| **esbuild** | `metafile.json` oder nur den Ausgabeordner | mit `--metafile` nein | mit `--metafile` ja |
| **Vite** | Ausgabeordner und seine Source Maps | ja | erfordert `--stats`-Ausgabe |
| **Rollup** | Ausgabeordner und seine Source Maps | ja | erfordert `stats.json` |
| **Parcel** | Ausgabeordner und seine Source Maps | ja | erfordert `stats.json` |
| **tsup / esbuild-Wrapper** | Ausgabeordner und seine Source Maps | ja | erfordert `stats.json` |
| **Angular / Next.js / Nuxt / SvelteKit** | was sie erzeugen, also webpack- oder vite-Ausgabe | je nach Fall | je nach Fall'
## Das Problem

Man lässt einen Bundle-Analyzer auf einen Build los, und er läuft in den
Speichernotstand oder braucht eine Minute und liefert ein Bild, mit dem man
nichts anfangen kann. `webpack-bundle-analyzer` hält die gesamte `stats.json` im
JS-Heap: bei einem 363-MB-Build sind das **63,6 Sekunden und 2,3 GB**, um die
Frage „wie groß ist das?“ zu beantworten. Und selbst wenn der Treemap gezeichnet
ist, kann er die beiden Dinge nicht sagen, die wirklich Geld kosten — siehe
[unten](#ghost-code-und-versteckter-code).

Nopeat verarbeitet die Stats-Datei als Stream, misst die tatsächlich
erzeugten Bytes, fügt die Source Maps in den Modulgraphen ein und sagt, welche
Dimension verwendet wurde.

## Messwerte

Vollständige Pipeline — Stats parsen, jedes Asset auf der Platte messen, Source Maps
zusammenführen, Report rendern. Synthetische Fixtures, Median aus drei Läufen,
Referenzmaschine.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/chart-pipeline-dark.svg">
  <img alt="Balkendiagramme zum Vergleich von Nopeat und webpack-bundle-analyzer. Laufzeit: 1,87 s gegenüber 63,6 s bei einer 363-MB-Stats-Datei, 6,14 s gegenüber 176,3 s bei 1 GB. Speicher: 127 MB gegenüber 2.295 MB sowie 376 MB gegenüber 1.437 MB." src="docs/assets/chart-pipeline-light.svg" width="100%">
</picture>

| Eingabe | Nopeat | webpack-bundle-analyzer | Verhältnis |
|---|---|---|---|
| 363 MB `stats.json`, 154.379 Module | **1,87 s / 127 MB** | 63,6 s / 2.295 MB | **34× schneller, 18× kleiner** |
| 1 GB `stats.json`, 445.602 Module | **6,14 s / 376 MB** | 176,3 s / 1.437 MB | 29× schneller, 3,8× kleiner |

Source-Map-Zuordnung, aufgetragen über die Anzahl der Quellen in der Map. Das ist die
Superlinearität, die große Maps im Referenzwerkzeug unbrauchbar macht: **das
Fünffache an Daten kostet das Dreißigfache an Zeit**, während ours linear bleibt.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/chart-source-maps-dark.svg">
  <img alt="Log-log-Diagramme für Zeit und Speicher der Source-Map-Zuordnung über der Anzahl der Quellen. source-map-explorer steigt von 0,25 s beim realen preact-Build (12 Quellen) auf 562 s bei 50.000; Nopeat von 7 ms auf 209 ms. Speicher bei 50.000 Quellen: 642 MB gegenüber 67 MB." src="docs/assets/chart-source-maps-light.svg" width="100%">
</picture>

Jede Zahl hier ist mit der Harness in `bench/` reproduzierbar, und die Rohdaten sind
eingecheckt: [`docs/assets/charts-data.json`](docs/assets/charts-data.json) listet jede
Angabe mit ihrer Herkunft, das [Evidenz-Log](ARCHITECTURE.md) die Sitzungen
dahinter.

## Ghost-Code und versteckter Code

Die zwei Diagnosen, die keines der Referenzwerkzeuge liefern kann. Das ist das
eigentlich Neue an diesem Projekt.

- **Ghost-Code** — ein Modul, das der Bundler angegeben und ausgeliefert hat, das aber
  **keine Source Map erklärt**. Das Tree-Shaking hat es verpasst, oder das Asset wurde
  ohne Map gebaut. Es steckt im Bundle und in keinem Größenbericht.
- **Versteckter Code** — erzeugte Bytes, die zu **keinem** Modul gehören. Ein inline
  eingefügtes Stück, ein `eval`, ein injizierter Polyfill. Es steckt im Bundle und in
  keinem Modul.

Nopeat rechnet den Byte-Anteil jeder Quelle den Modulen zu, die sie erzeugt haben,
und alles, was nicht aufgeht, wird zu einer Diagnose statt zu einem Rundungsfehler:

```
$ nopeat ./dist
dist  ·  50000 modules  ·  1 assets  ·  0 packages  ·  ingest 2143 ms  ·  total 2845 ms  ·  dimension attributed
fusion: 1 map(s) · coverage 100% · 50000/50000 modules attributed · ghost code needs a stats.json to detect · 0 hidden source(s) (0 KB)
wrote dist/report.html (0.1 MB), detail in a companion script (loaded on demand)
```

und wenn es nicht aufgeht, sagt es in welche Richtung. Ein Build, bei dem nur eines von
zwei Assets eine Source Map mitbringt:

```
$ nopeat ./dist
dist  ·  22 modules  ·  2 assets  ·  11 packages  ·  ingest 4 ms  ·  total 6 ms  ·  dimension parsed
fusion: 1 map(s) · coverage 47% · 0/22 modules attributed · 22 ghost (83 KB of declared) · 3 hidden source(s) (39 KB)
```

Beachten Sie `dimension parsed`, nicht `attributed`: die Hälfte des Builds ist ohne Map,
also wurde die Ground-Truth-Dimension zurückgezogen, und der Report sagt das in seiner
ersten Zeile. Ein Werkzeug, das beides stillschweigend mittelt, würde eine Zahl melden,
die zu keiner Messung gehört.

Die Zuordnung erfolgt über das längste gemeinsame Pfad-Suffix auf dem echten
Join-Key ([`unified-graph.md`](docs/schema/unified-graph.md)), **niemals** über
Content-Hashes, und die Summe der korrigierten Größen wird per Invariante gegen die
Asset-Summe geprüft — bei Verstoß gibt `NPT0042` laut Fehler, statt still zu runden.

## Der Report

<p align="center">
  <img alt="Nopeat-HTML-Report: ein squarified Treemap eines Builds, nach Paket gruppiert, mit den 40 größten von 400 Paketen, durchsuchbarer Modulliste, drei Gruppierungsdimensionen und hellem und dunklem Design." src="docs/assets/treemap-large.svg" width="100%">
</p>

<sub>Die 40 größten Pakete eines synthetischen Builds mit 400 Paketen: 1.500 Assets und
8.041 Module, in den vom Bundler deklarierten Größen. Gezeichnet aus dem graph-Payload
des Werkzeugs von `bench/harness/render-treemap.py`, kein Screenshot, die Beschriftungen
sind also die Ausgabe des Werkzeugs; die CI erzeugt das Bild neu und schlägt bei einer
Abweichung fehl. Der HTML-Report bietet zusätzlich Suche, drei Gruppierungsdimensionen,
eine Detailansicht je Modul, Hell und Dunkel und keine Netzwerkanfragen.</sub>

## Installation

**Noch nicht veröffentlicht.** Es gibt kein `nopeat` auf npm und kein Crate zum
Installieren, also funktionieren `npx nopeat` und `cargo install nopeat-cli`
heute nicht, und diese README bietet sie nicht an. Aus dem Quellcode bauen:

```bash
git clone https://github.com/Nopeat/Nopeat.git
cd nopeat
cargo build --release
./target/release/nopeat ./dist
```

Nach einem `v*`-Tag veröffentlicht die Release-Pipeline in der Reihenfolge aus
`ARCHITECTURE.md release-and-ci.md` §2: zuerst Binärdateien in die GitHub-Releases, dann
`nopeat-core` auf crates.io, zuletzt den npm-Wrapper — der npm-Name ist die knappste
Ressource und wird zuletzt ausgegeben. Die npm- und crates.io-Badges und die beiden
Installationsbefehle kommen im Commit zurück, der `repo-links.json` ausfüllt. Der
npm-Wrapper prüft `checksums.txt`, bevor er etwas schreibt oder ausführt.

## Verwendung

```bash
nopeat ./dist                      # ein Ordner: stats + assets + *.map
nopeat ./dist/stats.json           # nur die stats-Datei
nopeat ./dist/metafile.json        # esbuild-Metafile

nopeat ./dist --budget nopeat.config.json   # Exit 1 bei Überschreitung
nopeat ./dist --mode json > sizes.json          # für CI oder BI
nopeat ./dist --mode csv  > sizes.csv
nopeat ./dist/map.js.map --bench-map            # nur Zuordnung, mit Zeitmessung
```

```jsonc
// nopeat.config.json
{
  "limits": [
    { "scope": "total",   "max": 1500000 },
    { "scope": "chunk",   "match": "vendor", "max": 800000 },
    { "scope": "package", "match": "moment", "max": 250000, "dimension": "gzip" }
  ]
}
```

<details>
<summary>Exit-Codes — Teil des Vertrags, denn CI-Gates bauen darauf</summary>

| Code | Bedeutung |
|---|---|
| 0 | analysiert, alle Budgets eingehalten |
| 1 | analysiert, ein Budget überschritten (`NPT0040`) |
| 2 | ungültige Kommandozeile |
| 3 | Eingabe unlesbar, oder eine Budgetregel trifft auf nichts |

Eine Regel, die auf nichts trifft, ist **absichtlich** ein Fehler: ein Tippfehler in
einer Regel darf CI nicht stillschweigend bestehen lassen.

</details>

## Stand

Vor 1.0, und das ist die ehrliche Tabelle. Was nicht gemessen wurde, steht als solches da.

| Bereich | Zustand |
|---|---|
| stats-Eingabe, Größenzuordnung, Source Maps, Fusion, Report, Budgets, JSON/CSV | implementiert, gemessen, per CI abgesichert |
| Laufzeit der 1-GB-Eingabe | **verfehlt**: 4,40 s gegen ein Ziel von 3 s (Speicher mit 350 MB unkritisch) |
| Erster Report-Paint / 30 fps | **unverifiziert** — kein Browser in CI; stattdessen gemessen: 1,56 MB und 1,27 s bei 154.379 Modulen |
| WASM-Build, WebGL-Renderer | nicht begonnen |
| Linux- / macOS-Runner | keine plattformspezifischen Verzweigungen; CI läuft nur unter Windows |
| tests | 82 Rust, 4 npm, 1,018 generated layout cases | alles grün |
| coverage | Abdeckung 82.19% of lines, against a 70% floor |

Die einzige Verfehlung steht mit Ursache im [CHANGELOG](CHANGELOG.md): es ist der
DOM-Cursor von `serde_json` über ein Modul-Array mit 445.602 Elementen.

## Warum

Jede Entwurfsentscheidung folgt einer Messung, keiner Vorliebe.

| Entscheidung | Beleg |
|---|---|
| Streamendes JSON, nicht `simd-json` | `simd-json` braucht das ganze Dokument im Speicher — genau die Decke, die wir abräumen ([ADR-0001](ARCHITECTURE.md)) |
| eigener HTML-Report, kein fremder Viewer | ein Viewer, den wir nicht kontrollieren, kann Fusion-Daten ohne Fork nicht zeigen ([ADR-0002](ARCHITECTURE.md)) |
| rayon für Größen und gzip | 25.600 Assets: 2.177 ms seriell → **342 ms** (6,4×); das Parsen war nie der Engpass |
| ein Suffix-Index für den Join | alle Quellen je Modul zu durchsuchen sind 2,5 Milliarden Vergleiche; der Index machte daraus 73,8 s → 4,9 s |
| exakte Zählungen, begrenzte Listen | ein Eintrag pro nicht gemapptem Modul kostete 47 MB für ein Feld, das *summary* heißt |
| Property-Tests statt Fixtures | zwei echte Fehler — Modulgrößen wurden **nie kleiner**, und ein Quellindex außerhalb der Liste ließ die Zuordnung still verschwinden — die kein Fixture im Repository sehen konnte |

### Was wir nicht gebaut haben, und warum

| verworfen | Grund |
|---|---|
| WBA-Viewer übernehmen | ohne Fork keine Fusion-Daten; mit Fork ist die UI unsere |
| `simd-json` | braucht das Dokument im Speicher, also genau das Problem |
| WebGL-Treemap in Phase 1 | 10k Knoten sind auf Canvas 2D in Ordnung; der Renderer-Wechsel ist Phase 2 hinter einem stabilen Payload |
| ein allgemeines Build-Werkzeug | gemessener Schmerz ist eine Speicherdecke, ein Prozess pro Element oder ein superlinearer Algorithmus. Hier liegt alle drei vor. |

## Dokumentation

- [Produktanforderungen](ARCHITECTURE.md) · [Evidenz-Log](ARCHITECTURE.md) · [Architektur](ARCHITECTURE.md)
- [Benchmarks und Ziele](docs/schema/bench-spec.md) · [Parität und Tests](docs/schema/bench-spec.md) · [Releases und CI](CONTRIBUTING.md)
- [Verträge](docs/schema/unified-graph.md) — Payload-Schema, CLI-Oberfläche, Benchmark-Protokoll, i18n-Regeln, Ownership
- [ADRs](ARCHITECTURE.md)  · [Risikoregister](docs/schema/bench-spec.md) · [Roadmap](docs/schema/bench-spec.md)

> Die deutsche Dokumentation wird übersetzt. Englisch ist die **normative Quelle**;
> `node bench/harness/check-i18n.mjs` prüft, dass die vier Sprachen nicht auseinanderlaufen.

## Mitmachen

Jede Änderung bringt eine Messung mit oder ein Argument, warum sie keine haben kann.
Siehe [CONTRIBUTING.md](CONTRIBUTING.md); die Gates sind `cargo test`,
`clippy -D warnings` (inkl. pedantic), `cargo fmt`, eine Coverage-Untergrenze von
82 %, Parität gegen `webpack-bundle-analyzer` und eine Vier-Sprachen-Dokumentprüfung.
[Verhaltenskodex](CODE_OF_CONDUCT.md) · [Sicherheitsrichtlinie](SECURITY.md)

## Lizenz

MIT ([LICENSE-MIT](LICENSE-MIT)) oder Apache-2.0
([LICENSE-APACHE](LICENSE-APACHE)), nach Ihrer Wahl.

---

<div align="center">
  <sub>
    öffentlich gebaut. Einwände zu den Zahlen sind willkommen in den
    <a href="https://github.com/Nopeat/Nopeat/issues">Issues</a> — sie sind das
    Einzige, was einen Benchmark bedeutsam hält.
  </sub>
</div>
