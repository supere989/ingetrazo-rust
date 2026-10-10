# IngeTrazo (Rust Native Implementation)

**High-performance, pure Rust port of IngeTrazo — 3D Push/Pull Architectural and CAD Modeler.**

Rust port of the original Python/Qt project. Feature and usability parity are still subject to validation.

Based on [IngeTrazo](https://github.com/ingelibre/ingetrazo) by Marco Sumari Tellez and contributors. Original source and packaging assets are retained as the `ingetrazo/` submodule from [supere989/ingetrazo](https://github.com/supere989/ingetrazo). This port is licensed under GPL-3.0-or-later; see `LICENSE` and `AUTHORS`.

Clone with the original assets:

```bash
git clone --recurse-submodules https://github.com/supere989/ingetrazo-rust.git
cd ingetrazo-rust
```

For an existing checkout, run `git submodule update --init --recursive`.

## Workspace Architecture

The codebase is partitioned into 6 focused, high-cohesion crates under `crates/`:

```
ingentrazo-RUST-Conversion/
├── Cargo.toml (Workspace)
├── crates/
│   ├── ingetrazo-core/        # Shared-vertex non-manifold mesh, PointKey spatial welding (~0.1mm),
│   │                          # Newell normal & area, Ray/Plane/AABB math, OrbitCamera (2-point perspective),
│   │                          # Snapping & Inference engine, Transactional Undo/Redo, Units & VCB parser,
│   │                          # Groups/Components, Layers/Tags, Materials, BIM IFC4 tagging & takeoff
│   ├── ingetrazo-tools/       # 13 Interactive modeling tools: Line, Rectangle, Rotated Rectangle,
│   │                          # Circle, Arc (2-pt & 3-pt), Push/Pull, Offset, Move/Copy (array repeat),
│   │                          # Rotate (protractor), Scale, Eraser, Tape Measure, Select, Paint
│   ├── ingetrazo-formats/     # Native IGZ format (Format 1 JSON & Format 2 ZIP with embedded textures),
│   │                          # Wavefront OBJ (import/export), Stereolithography STL (binary & ASCII),
│   │                          # glTF 2.0 / GLB binary exporter, AutoCAD DXF (3DFACE & polylines)
│   ├── ingetrazo-render/      # Viewport render pipeline: vertex/edge GPU batching, HLR hidden line styling,
│   │                          # Visual styles (Shaded, Wireframe, Hidden line, Monochrome, X-Ray),
│   │                          # Selection highlighting and inference snap marker glyphs
│   ├── ingetrazo-gui/         # Native desktop GUI application via eframe / egui:
│   │                          # Menu bar, Main Toolbar, Side trays (Materials, Layers, Entity Info),
│   │                          # Status hints, Value Control Box (VCB), interactive 3D Viewport canvas
│   └── ingetrazo-cli/         # Command-line entry point: GUI app runner, --check diagnostics,
│                              # Headless format converter (convert <input> <output>)
```

## Quick Start

### Build & Test

```bash
# Run the entire test suite across all crates
cargo test --workspace

# Build the release binary
cargo build --release
```

### Run Diagnostics

```bash
cargo run -p ingetrazo-cli -- --check
```

### Launch Native Desktop Application

```bash
cargo run --release -p ingetrazo-cli
# Or open a model directly:
cargo run --release -p ingetrazo-cli -- path/to/model.igz
```

### Headless File Format Conversion

```bash
cargo run --release -p ingetrazo-cli -- convert input.igz output.obj
cargo run --release -p ingetrazo-cli -- convert input.igz output.stl
```

### Packaging into Application

Build both the standalone AppImage and the portable tarball with one script:

```bash
./package.sh
```

Generated packages will be in `dist/`:
- **AppImage**: `dist/IngeTrazo-0.3.15-x86_64.AppImage` (6.3 MB single-file executable)
- **Portable Tarball**: `dist/IngeTrazo-0.3.15-linux-x86_64.tar.gz` (5.9 MB with launcher and icon)

### Local Desktop Installation

To install IngeTrazo into your local user applications menu with `.igz` desktop integration:

```bash
./install-local.sh
```

## Supported Modeling Operations & Tools

- **Line (`L`)**: Click-to-click edge drafting, rubber-band preview, VCB length input, automatic face closing upon completing planar loops.
- **Rectangle (`R`)**: 2-click planar rectangle on XY, XZ, YZ, or face planes with dimension VCB input (`width, height`).
- **Push/Pull (`P`)**: Extrude faces along Newell normals, recessing, punch-through detection, double-click repeat last distance.
- **Circle (`C`)** & **Arc (`A`)**: Configurable segment counts (`24s`), 2-point bulge arc and 3-point circular arcs.
- **Move / Copy (`M`)**: Selected entity translation, Ctrl modifier to duplicate, array repeat multiplier (`5x`, `/3`).
- **Rotate (`Q`)**: 3-click protractor workflow (origin, baseline, angle), copy mode.
- **Scale (`S`)**: 9-point grip scaling with uniform modifier.
- **Tape Measure (`T`)**: Dimension readouts, guides and point snaps.
- **Eraser (`E`)**: Edge removal, soften/smooth edge modifier.
- **Paint (`B`)**: Apply materials and colors, Alt eyedropper sampling.
- **Selection (`Space`)**: Single click, window selection, double-click (face + boundary), triple-click (connected solid).

## Verification & Parity

See the [parity progress tracker](docs/PARITY_PROGRESS.md) for the audited inventory, open gaps, release gates and reproducible evidence.

The latest local reviewed integration passes 296 Rust tests and adds original-runtime saved-view recall, live dimension anchors, nested world measurement and outliner checks. glTF/GLB imports and corrected Y-up exports pass independent probes; face intersection and healing have original geometry comparisons. Four original engineering documents retain field/asset preservation and Python reopening. Full interactive and feature parity remains open; see the tracker for acceptance boundaries and current gaps.

