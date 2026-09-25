# TPT CI Config Validator

CI-pipeline-friendly config validation against schemas and custom rules —
Kubernetes manifests, Terraform, GitHub Actions / generic CI configs.

- **Free (this repo's WASM build):** one file at a time, in-browser, at
  `tptsolutions.co.nz/tools/ci-config-validator`.
- **Pro edition ($29 USD, one-time team license, Gumroad):** batch/directory
  validation, custom rules, and a CI CLI binary with proper exit codes.
  See `GUMROAD.md` for the listing copy.

## Layout

| Path        | What                                                                      |
| ----------- | ------------------------------------------------------------------------- |
| `src/`      | wasm DOM app (`tpt-appfront`); hub contract: `mount(container)` + `start()` |
| `engine/`   | target-independent validation engine (`tpt-config-engine`) — no UI deps    |
| `cli/`      | Pro CI CLI binary (`tpt-cicfg-cli`) — exit 0 pass / 1 violations / 2 error |
| `desktop/`  | Pro desktop shell (`tpt-cicfg-desktop`, `tpt-appfront-webview`)            |

## Build

```pwsh
./build.ps1          # free WASM edition -> dist\web (or $TPT_WEB_REPO\public\apps\ci-config-validator)
./build.ps1 -Pro     # pro: trunk bundle + desktop exe + CLI
./build.ps1 -SkipBuild  # re-run wasm-bindgen only
```

`just dev` runs the standalone trunk dev server (mounts into the
`#tpt-appfront-root` marker div in `index.html`; the hub embeds the same wasm
and calls `mount(container)` itself, so the two hosts never double-mount).

The wasm-bindgen crate is pinned `=0.2.128` to the installed `wasm-bindgen`
CLI version — bump both together.

## CI CLI (Pro)

```text
tpt-cicfg-cli [OPTIONS] <PATH>...
  --rules <FILE>    custom rules file (YAML/JSON) — see examples/custom-rules.yaml
  --format <FMT>    text (default) | json
Exit codes: 0 pass · 1 violations (errors or parse failures) · 2 usage/IO error
```

Files or whole directories (`.yaml .yml .json .xml .tf .hcl`, walked
recursively, VCS/build dirs skipped). Warnings never fail the run.

Rule-file note: quote any `select:` path containing `[` — a bare `*` in an
unquoted YAML scalar reads as an alias indicator
(`select: "spec.template.spec.containers[*].image"`).

## Hub contract

`wasm-bindgen --target web` glue exposes `mount(container)`, called by the
hub's `WasmAppRunner` after `default()` init. Registry entry (hub repo
`tpt-electrician-nz-2`, `src/lib/tpt-apps-registry.ts`): slug
`ci-config-validator`, category `developer`,
`wasm: { entry: '/apps/ci-config-validator/ci-config-validator.js' }`.
