# erdcat

CLI tool that reads any SQLite file and prints its schema as an ER diagram in plain text:
Unicode box-drawing by default (`--format ascii` for `+ - |` fallback), plus emitters for
`dot`, `mermaid`, and later `d2`. Pure CLI — no TUI, no watch mode in v1.

## Status

**M1 complete (2026-08-21).** Introspection + dot/mermaid emit work end-to-end.

- `src/schema.rs` — read-only introspection via rusqlite (`sqlite_master`, `PRAGMA table_info`,
  `PRAGMA foreign_key_list`); junction-table detection; FK target resolution.
- `src/emit/dot.rs` — graphviz output: HTML-like record labels, per-column ports,
  crow's-foot m:n edges for junctions.
- `src/emit/mermaid.rs` — mermaid `erDiagram` output.
- `src/main.rs` — clap CLI: `erdcat <db> [--format dot|mermaid]` (default dot).
- `tests/golden.rs` — 3 integration tests (fixture DB in memory).
- Validated on real data: `~/.emacs.d/org-roam.db` → 13 tables, 6 FK edges, junctions collapsed.

Milestone history:

- **M1 (done)** — introspection + dot/mermaid emit. Hiccups fixed along the way: a missing
  module file, a borrow conflict in FK resolution (fixed collect-then-apply), junction edge
  direction nondeterminism in tests (FK list order is reverse-declaration), untyped-column
  rendering (org-roam declares no column types).
- **M2 (next)** — Unicode happy-path renderer (see Roadmap).

## Algorithms & libraries

| Stage | Algorithm | Library |
|---|---|---|
| SQLite access | read-only open, catalog queries | `rusqlite` 0.40 (`bundled` feature, `SQLITE_OPEN_READ_ONLY`) |
| CLI | derive API | `clap` 4.6 (`derive`) |
| Schema graph | petgraph-compatible model built in `schema.rs` | std only for now |
| Layout (M2) | Sugiyama layered DAG: rank assignment → crossing reduction → coordinates | `dagre` crate v0.1.1 (<https://github.com/kookyleo/dagre-rs>, Apache-2.0, petgraph-based; handles self-loops + multi-edges) |
| Edge routing (M2/M3) | dagre polylines, then port-snapping A* re-route on the character grid (snap endpoints to box ports, orthogonal paths, avoid box interiors) | own code on top of dagre output |
| Rasterization (M2/M3) | char-grid painter: box-drawing glyphs (`─ │ ┌ ┐ └ ┘ ├ ┤ ┬ ┴`) with crossing merge to `┼` | own code |

Known dagre caveat: it has **no true port constraints**, hence our snapping/A* pass after it.
Fallbacks if `dagre` 0.1.1 proves unusable: `dagre-dgl-rs`, or vendor the algorithm.

Locked design decisions:

- Junction heuristic: exactly 2 FKs, zero non-PK non-FK columns, two distinct targets →
  collapse table into one m:n edge between the targets.
- FK with implicit target column resolves to target's first PK column, else `"rowid"`.
- Focus-mode centering (M4): **Option A hop-ranked layers** — BFS hop distance from seed;
  parents above, children below, seed horizontally centered.
- Legibility ceiling: ≤15 tables renders well; 40+ requires focus mode.

## Roadmap

- **M2 — Unicode happy path:** add `dagre` dep; lay out schema graph; draw table boxes at node
  coords; route edges orthogonally with port snapping; rasterize glyphs. `--format unicode`
  becomes default, `ascii` variant uses `+ - |`. Keep dot/mermaid working.
- **M3 — router hardening:** crossing merging (`┼`), label placement without overlap,
  self-referential FK loops, wide/deep graph stress, golden-image tests (compare rendered
  grids byte-for-byte against fixtures).
- **M4 — focus & polish:** `--tables X --depth N` (hop-ranked layers, truncation footer
  "… N more tables within depth"), `--row-counts`, compact mode, fuzzy table picker
  (may defer to post-v1). `--format d2` emitter.

## Environment

- NixOS host `saif-thinkpad`. Rust 1.98.0 + clippy + rustfmt are installed **globally**
  via `/etc/nixos/users/saifr/home.nix` (oxalica `rust-overlay`). Plain `cargo` works;
  this repo's `flake.nix` is a stub empty devShell — do not require `nix develop`.
- Real-world test DB: `~/.emacs.d/org-roam.db`.
- Verify before claiming done: `cargo test && cargo clippy --all-targets && cargo fmt --check`.

## Conventions

- No comments in code unless asked.
- Never commit unless explicitly asked.
- User prefers terse, direct answers.
