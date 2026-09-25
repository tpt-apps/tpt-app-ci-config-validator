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
- [x] Survey the three base crates for existing YAML/XML/JSON parsing and
      schema-validation support (Kubernetes/Terraform config shapes)
      (`tpt-yaml-core`: arena node model, multi-doc, anchors/aliases — used;
      `tpt-data-parsers` has no general JSON/XML parser — its JSON crate is
      streaming JSONL; `tpt-xml` does not exist — XML is a vendored minimal
      parser in `engine/src/xml.rs`, JSON is serde_json)
- [x] Adapter crate `engine/`: config file(s) + schema/rule set in →
      validation findings (path, rule violated, message) out as plain
      structs — no UI deps, host-unit-testable
      (`tpt-config-engine`: normalized `Value` tree, path selectors,
      conditions, `validate`/`validate_source`; 45 unit + 12 fixture tests)
- [x] Custom-rule format: define a simple rule DSL/schema format users can
      author without learning a new language (`examples/custom-rules.yaml`;
      conditions: exists, missing, equals, not_equals, matches,
      not_matches, contains, gt/gte/lt/lte, one_of, is_a, has_key)
- [x] Validation against real-world sample Kubernetes manifests and Terraform
      configs (valid and deliberately broken)
      (`engine/tests/fixtures/` — K8s/GHA/Terraform/pom.xml valid + broken;
      GitHub Actions and Maven pom packs included; Terraform is a line lint,
      not tree rules)

## Phase 2 — Free WASM UI
- [x] Single-file upload/paste + validation against built-in schema rules
      (appfront pattern from fea-lite: static shell mounted once, results
      re-render via `tpt_appfront_dom::render`; hidden `<input type=file>` +
      FileReader drives Upload; embedded broken-sample button for instant try)
- [x] Free-tier upsell copy pointing at batch/directory validation, custom
      rules, CI CLI, team license (dashed upsell card; `pro` build swaps it
      for a custom-rules panel + multi-file batch input)
- [x] Browser-verified: mount, validate, findings render correctly
      (trunk serve + in-app browser: broken sample → 6 findings with
      severity chips/paths/messages; valid deployment → "✓ All checks
      passed". Two bugs found & fixed: file-input wiring ran before the
      shell existed; results view had >1 root child which made
      `into_only_child` panic — with panic=abort that traps wasm, so views
      must always produce exactly one root)

## Phase 3 — Paid desktop edition (cargo feature `pro`)
- [x] `pro` gates: batch/directory validation, custom rule authoring, a CI
      CLI binary with proper exit codes (0 = pass, non-zero = violations),
      team license (not metered/per-seat)
      (free wasm build compiles without `pro`; CLI ships only in Pro builds)
- [x] Pro UI: directory picker, custom-rule editor, batch results table
      (multi-file `<input multiple>` batch + custom-rules paste panel;
      per-file result cards. Native directory picker left to the CLI —
      the webview can't host one portably)
- [x] CLI binary: separate `bin/` target reusing the same `engine/` crate,
      with `--config`/`--rules`/`--format` flags and CI-friendly output
      (`cli/` → `tpt-cicfg-cli`; positional paths + `--rules` + `--format
      text|json`; exit 0 pass / 1 violations / 2 usage-or-IO; recursive
      directory batching; smoke-tested against all fixtures)
- [x] Desktop shell via `tpt-appfront-webview` for the UI; CLI built and
      smoke-tested standalone (this is the one app where the CLI matters as
      much as the GUI)
      (`desktop/` → `tpt-ci-config-validator-pro.exe`, embedded dist via
      include_dir, app:// protocol, open_external ACL — mirrors fea-lite)
- [ ] Gumroad product from apps.md listing copy ($29, team license), exe/CLI
      upload, URL into registry — manual step, listing copy at apps.md §T2-10
      (copy ready in GUMROAD.md; upload is manual)

## Phase 4 — Ship & measure
- [x] Registry entry (`developer` category, `wasm: { entry: '/apps/ci-config-validator/ci-config-validator.js' }`)
      per the "Adding a finished app to the hub" runbook at the bottom of apps.md
      (added as `coming-soon` with proFeatures; build.ps1 writes the free
      bundle to `public/apps/ci-config-validator/` — 932 KB raw / 334 KB gz)
- [x] Verify hub card/detail/sitemap/runner in dev, then prod after deploy
      (dev verified: `/tools/ci-config-validator` renders the card + demo,
      the hub `WasmAppRunner` mounts the app and validation runs inside it
      (broken sample → 6 findings), `/sitemap.xml` lists the page. Prod
      check remains for the next deploy. Note: the `/tools` listing route
      404s in this repo's dev server for ALL apps incl. fea-lite —
      pre-existing, not ours)
- [x] WASM size budget check on the shipped build (334 KB gz against the
      ~2 MB budget — regex + YAML/JSON/XML engine included)
