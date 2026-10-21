# TPT CI Config Validator - build pipeline (Windows).
#
# Free edition (default):  ./build.ps1
#   cargo build --release --target wasm32-unknown-unknown (no pro feature)
#   wasm-bindgen --target web  ->  web repo public/apps/ci-config-validator/
#   when TPT_WEB_REPO is set (glue named ci-config-validator.js, wasm keeps
#   its _bg.wasm name - the glue resolves it via import.meta.url), otherwise
#   dist\web.
# Pro desktop edition:     ./build.ps1 -Pro
#   trunk build --release --features pro (its own independent wasm build,
#   into dist\ — NOT the wasm-bindgen path above) + cargo build the native
#   webview exe + the CI CLI binary. Deliberately does not touch
#   public/apps/ci-config-validator/: that is the free in-browser demo's
#   bundle, and it must never be built with --features pro, or the site
#   would give away every paid feature (batch/directory validation, custom
#   rules) for free. The two editions are separate cargo builds writing to
#   separate directories on purpose — do not try to "reuse" one build for
#   both.
#
# Env:
#   TPT_WEB_REPO  path to the tptsolutions.co.nz web repo; the app is copied
#                 to <repo>\public\apps\ci-config-validator\ for the hub
#                 runner. Defaults to the known local checkout below if unset.

[CmdletBinding()]
param(
    # Build the Pro desktop bundle (--features pro) instead of the free one.
    # Never writes to the free edition's $Dest — see header comment.
    [switch]$Pro,
    # Free-edition only: skip the cargo build and re-run wasm-bindgen against
    # an existing release build.
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
Set-Location -LiteralPath $PSScriptRoot

$Crate = "tpt-app-ci-config-validator"
$Target = "wasm32-unknown-unknown"
$OutName = "ci-config-validator"
$Slug = "ci-config-validator"
$DefaultWebRepo = "D:\Programming\2 WIP\TPT Electrician\tpt-electrician-nz-2"

Write-Host "== TPT CI Config Validator build ==" -ForegroundColor Cyan

if ($Pro) {
    Write-Host " edition: pro (batch/directory validation + custom rules + CI CLI + team license)"
    Write-Host "-- trunk build --release --features pro (standalone bundle for the desktop shell)" -ForegroundColor Cyan
    trunk build --release --features pro
    if ($LASTEXITCODE -ne 0) { throw "trunk build failed" }

    Write-Host "-- cargo build --release -p tpt-cicfg-desktop (webview exe)" -ForegroundColor Cyan
    cargo build --release -p tpt-cicfg-desktop
    if ($LASTEXITCODE -ne 0) { throw "desktop build failed" }
    Write-Host "   exe: target\release\tpt-ci-config-validator-pro.exe (serves dist\)" -ForegroundColor Cyan

    Write-Host "-- cargo build --release -p tpt-cicfg-cli (CI CLI)" -ForegroundColor Cyan
    cargo build --release -p tpt-cicfg-cli
    if ($LASTEXITCODE -ne 0) { throw "cli build failed" }
    Write-Host "   exe: target\release\tpt-cicfg-cli.exe" -ForegroundColor Cyan

    # Package the Gumroad upload: release\tpt-ci-config-validator-pro-v<version>-win64.zip
    Write-Host "-- packaging the Gumroad zip" -ForegroundColor Cyan
    $Version = (Select-String -Path (Join-Path $PSScriptRoot "desktop\Cargo.toml") -Pattern '^version\s*=\s*"(.+)"').Matches[0].Groups[1].Value
    $Stage = Join-Path $PSScriptRoot "release\stage"
    $ZipPath = Join-Path $PSScriptRoot "release\tpt-ci-config-validator-pro-v$Version-win64.zip"
    if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $Stage | Out-Null
    Copy-Item (Join-Path $PSScriptRoot "target\release\tpt-ci-config-validator-pro.exe") $Stage
    Copy-Item (Join-Path $PSScriptRoot "target\release\tpt-cicfg-cli.exe") $Stage
    Copy-Item (Join-Path $PSScriptRoot "examples\custom-rules.yaml") $Stage
    @"
TPT CI Config Validator Pro v$Version
=====================================

tpt-ci-config-validator-pro.exe   Desktop app: double-click to run. Batch
                                  validation and custom rules. Needs the
                                  WebView2 runtime (preinstalled on
                                  Windows 10 20H2+ and Windows 11).
tpt-cicfg-cli.exe                 CI command-line validator.
custom-rules.yaml                 Example custom rules file (documents the format).

CLI usage:
  tpt-cicfg-cli [OPTIONS] <PATH>...
    --rules <FILE>    custom rules file (YAML/JSON)
    --format <FMT>    text (default) | json
  Exit codes: 0 pass, 1 violations, 2 usage/IO error

  Example:  tpt-cicfg-cli deploy/ --rules custom-rules.yaml

More developer tools: https://tptsolutions.co.nz/tools
"@ | Set-Content -Path (Join-Path $Stage "README.txt") -Encoding UTF8
    if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }
    Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $ZipPath
    Remove-Item $Stage -Recurse -Force
    Write-Host "   zip: $ZipPath" -ForegroundColor Green
    Write-Host "-- done" -ForegroundColor Green
    Write-Host "   (public/apps/ci-config-validator/ was not touched — run ./build.ps1 with no flags to refresh the free demo)"
    exit 0
}

# Where the browser-loadable free bundle goes.
$WebRepo = if ($env:TPT_WEB_REPO) { $env:TPT_WEB_REPO } elseif (Test-Path $DefaultWebRepo) { $DefaultWebRepo } else { $null }
$Dest = if ($WebRepo) {
    Join-Path $WebRepo "public\apps\ci-config-validator"
} else {
    Join-Path $PSScriptRoot "dist\web"
}

Write-Host " edition: free (single-file, built-in schema rules)"
Write-Host " dest:    $Dest"

if (-not $SkipBuild) {
    Write-Host "-- cargo build --release --target $Target" -ForegroundColor Cyan
    cargo build --release --target $Target -p $Crate
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$Wasm = Join-Path $PSScriptRoot "target\$Target\release\tpt_app_ci_config_validator.wasm"
if (-not (Test-Path $Wasm)) { throw "missing $Wasm - run the cargo build step first" }

Write-Host "-- wasm-bindgen --target web --out-name $OutName" -ForegroundColor Cyan
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
wasm-bindgen --target web --out-name $OutName --out-dir $Dest $Wasm
if ($LASTEXITCODE -ne 0) { throw "wasm-bindgen failed" }

$Glue = Join-Path $Dest "$OutName.js"
$BgWasm = Join-Path $Dest "$($OutName)_bg.wasm"
foreach ($artifact in @($Glue, $BgWasm)) {
    if (-not (Test-Path $artifact)) { throw "wasm-bindgen did not produce $artifact" }
}

# Payload size report (todo Phase 4: budget ~2 MB raw).
$raw = (Get-Item $BgWasm).Length
Add-Type -AssemblyName System.IO.Compression
$rawBytes = [System.IO.File]::ReadAllBytes($BgWasm)
$memoryStream = [System.IO.MemoryStream]::new()
$gzipStream = [System.IO.Compression.GZipStream]::new($memoryStream, [System.IO.Compression.CompressionLevel]::Optimal)
$gzipStream.Write($rawBytes, 0, $rawBytes.Length)
$gzipStream.Dispose()
$gzipped = $memoryStream.ToArray().Length
$memoryStream.Dispose()
$gzKb = [math]::Round($gzipped / 1KB, 1)
Write-Host "-- $(Split-Path $BgWasm -Leaf): $raw bytes raw, $gzipped bytes gzipped ($gzKb KB gz)" -ForegroundColor Cyan
if ($gzipped -gt 2MB) {
    Write-Warning "payload exceeds the ~2 MB gzip budget"
}

Write-Host "-- done" -ForegroundColor Green
if (-not $WebRepo) {
    Write-Host "   (set TPT_WEB_REPO to copy the bundle into the hub web repo)"
} else {
    Write-Host ""
    Write-Host "Paste into src/lib/tpt-apps-registry.ts (TPT_APPS_REGISTRY), then flip status to 'live':" -ForegroundColor Cyan
    Write-Host @"
  {
    slug: '$Slug',
    name: 'TPT CI Config Validator',
    description: 'Validate Kubernetes/Terraform/CI configs against schemas and custom rules.',
    category: 'developer',
    status: 'coming-soon',
    wasm: { entry: '/apps/$Slug/$OutName.js' },
    gumroadUrl: 'https://tpt12.gumroad.com/l/tpt-ci-config-validator',
  },
"@
}
