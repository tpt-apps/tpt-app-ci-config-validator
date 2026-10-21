# Gumroad listing

Everything needed to create/update the Gumroad product once this app is
built. Source copy is `apps.md` §T2-10 in the `tpt-electrician-nz-2` repo — if
that changes, update here too (and vice versa).

## Step-by-step: publish this listing

1. Build the Pro edition (`./build.ps1 -Pro`, once the build pipeline exists
   — see `todo.md` and mirror `tpt-app-fea-lite`/`tpt-app-cutlist-optimizer`'s
   pattern) — produces the packaged exe/zip in `release/`.
2. Log in to Gumroad → **New product** → **Digital product**.
3. Set the product name, price, and upload the packaged file (all below).
4. Paste the short description, bullets, and full description (below) into
   their respective fields.
5. Set category/tags (below) and add a cover image (see note below).
6. Publish. The product URL is `https://tpt12.gumroad.com/l/tpt-ci-config-validator`
   (set the custom permalink to `tpt-ci-config-validator`).
7. Paste that URL into the `ci-config-validator` entry's `gumroadUrl` field
   in `tpt-electrician-nz-2\src\lib\tpt-apps-registry.ts`, and set
   `price: '$29'` on the same entry.
8. Flip that entry's `status` to `'live'` once both the free demo and this
   Gumroad listing exist.
9. Commit and push the registry repo.

## File to upload

`release	pt-ci-config-validator-pro-v0.1.0-win64.zip` — produced by
`./build.ps1 -Pro` (desktop exe + CI CLI + example rules + README.txt). The
`release/` folder is gitignored; rebuild it before each upload.

## Product name

```
TPT CI Config Validator — One-Time-Purchase Config Validation for CI
```

## Price

**$29 USD, one-time purchase — team license** (not metered SaaS).

## Short description / summary field

```
Validate configs against your own rules without an ongoing SaaS subscription — one license, your whole team.
```

## Bullets / feature list

```
Team license, not metered SaaS
CI-ready CLI
Custom schema rules
Batch validation
One-time purchase
```

## Full description (long-form field)

```
TPT CI Config Validator is a CLI + desktop batch validator that checks
config files against schemas and custom rules, designed to drop into a CI
pipeline as a one-time-purchased binary rather than a metered SaaS call.

Who it's for: DevOps/platform engineers at small agencies and startups
validating large Kubernetes/Terraform/CI config files. Free formatters
handle one file in a browser tab; Datree and Spectral Cloud solve the
CI-integration problem but charge an ongoing SaaS subscription for what is
fundamentally a validation pass.

Pro edition (this download) — sold as a team license:
- Batch/directory validation (free is one file at a time)
- Custom schema rules
- CI-pipeline-friendly CLI with exit codes
- Usable by the whole team under one license

Try the free single-file edition first, right in your browser, no install:
tptsolutions.co.nz/tools/ci-config-validator

Requirements: Windows 10 (20H2+) or Windows 11 for the desktop app; the CLI
runs anywhere your CI pipeline does. Uses the WebView2 runtime, preinstalled
on both — nothing extra to download.
```

## Category / tags

- Category: **Software > Developer Tools** (or Gumroad's closest equivalent)
- Tags: `devops`, `ci-cd`, `config-validation`, `kubernetes`, `terraform`,
  `windows`

## Cover image / thumbnail

Real app screenshots live in `screenshots/` (`00-cover-1280x720.png` is
cover-sized; `03-batch-validation.png` is the strongest listing shot).
No stock/AI art.

## After publishing

1. The product URL is `https://tpt12.gumroad.com/l/tpt-ci-config-validator`.
2. Paste it into the `gumroadUrl` field of the `ci-config-validator` entry
   in `tpt-electrician-nz-2\src\lib\tpt-apps-registry.ts`, and set
   `price: '$29'`.
3. Set `status` to `'live'`.
4. Commit and push the registry repo.
