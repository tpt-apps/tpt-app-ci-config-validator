# TPT CI Config Validator — Build Todo

> Hub listing: `tptsolutions.co.nz/tools/ci-config-validator` (registry slug
> `ci-config-validator`, category `developer`). Spec: `apps.md` §T2-10 in the
> tpt-electrician-nz-2 repo. Base wraps `tpt-yaml` + `tpt-xml` +
> `tpt-data-parsers`. Free version: one file at a time, in-browser. Paid
> offline edition ($29 USD, Gumroad, team license): batch/directory
> validation, custom rules, CI CLI with exit codes, team license. UI on
> `tpt-appfront`; pro edition also ships a CLI binary for CI use.
>
> Hub contract: `wasm-bindgen --target web` glue exposes `mount(container)`
> (called by `WasmAppRunner` after `default()` init). Standalone `trunk serve`
> mounts into the `#tpt-appfront-root` marker div instead — never
> `document.body` — so the two hosts never double-mount.

## Phase 0 — Scaffold & pipeline proof
- [x] `tpt-appfront init tpt-app-ci-config-validator --target dom` (path deps on the tpt-appfront checkout)
      (dir pre-existed with docs, so the scaffold was generated from the same
      `--target dom` template and merged by hand)
- [x] Restructure scaffold to hub contract: `#[wasm_bindgen] pub fn mount(el)` + `#[wasm_bindgen(start)]` no-op unless `#tpt-appfront-root` present
- [x] `index.html` carries the `#tpt-appfront-root` marker div
- [x] Build pipeline script mirroring fea-lite/cutlist-optimizer's (`--target web` glue → web repo `public/apps/ci-config-validator/`)
      (`build.ps1` / `build.sh` / `justfile`; hello-world release build +
      wasm-bindgen verified into `dist/web`, 27 KB wasm)
- [x] Hello-world build loads through the standalone dev page (`trunk serve`)
- [x] git init + first commit
- [x] Note: base repos `tpt-yaml` and `tpt-xml` not found under `Open Source/`
      at planning time — confirm location/name before Phase 1;
      `tpt-data-parsers` is present
      (resolved: `tpt-yaml` now exists under `Open Source/` since 2026-09-21;
      `tpt-xml` still does not exist anywhere — Phase 1 uses `tpt-yaml` +
      `tpt-data-parsers`, XML via a vendored minimal parser or a crates.io
      dependency)

## Phase 1 — Core engine (wrap tpt-yaml + tpt-xml + tpt-data-parsers)
- [ ] Survey the three base crates for existing YAML/XML/JSON parsing and
      schema-validation support (Kubernetes/Terraform config shapes)
- [ ] Adapter crate `engine/`: config file(s) + schema/rule set in →
      validation findings (path, rule violated, message) out as plain
      structs — no UI deps, host-unit-testable
- [ ] Custom-rule format: define a simple rule DSL/schema format users can
      author without learning a new language
- [ ] Validation against real-world sample Kubernetes manifests and Terraform
      configs (valid and deliberately broken)

## Phase 2 — Free WASM UI
- [ ] Single-file upload/paste + validation against built-in schema rules
- [ ] Free-tier upsell copy pointing at batch/directory validation, custom
      rules, CI CLI, team license
- [ ] Browser-verified: mount, validate, findings render correctly

## Phase 3 — Paid desktop edition (cargo feature `pro`)
- [ ] `pro` gates: batch/directory validation, custom rule authoring, a CI
      CLI binary with proper exit codes (0 = pass, non-zero = violations),
      team license (not metered/per-seat)
- [ ] Pro UI: directory picker, custom-rule editor, batch results table
- [ ] CLI binary: separate `bin/` target reusing the same `engine/` crate,
      with `--config`/`--rules`/`--format` flags and CI-friendly output
- [ ] Desktop shell via `tpt-appfront-webview` for the UI; CLI built and
      smoke-tested standalone (this is the one app where the CLI matters as
      much as the GUI)
- [ ] Gumroad product from apps.md listing copy ($29, team license), exe/CLI
      upload, URL into registry — manual step, listing copy at apps.md §T2-10

## Phase 4 — Ship & measure
- [ ] Registry entry (`developer` category, `wasm: { entry: '/apps/ci-config-validator/ci-config-validator.js' }`)
      per the "Adding a finished app to the hub" runbook at the bottom of apps.md
- [ ] Verify hub card/detail/sitemap/runner in dev, then prod after deploy
- [ ] WASM size budget check on the shipped build
