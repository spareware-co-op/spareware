# Builds the Transport command layer for the browser (WebAssembly, target web) into
# projects/spare-construct/demo/transport/pkg, with no network: cargo runs --offline from its cache, and the wasm-bindgen
# command line is the one wasm-pack already cached (it must be 0.2.118, the version the crate is built with).
# Then check the module against the prototype: node tools/transport-truth/wasm-check.mjs
#
# Run from the repository root:  powershell -File tools/transport-truth/build-wasm.ps1
# Set CARGO_TARGET_DIR first to build outside the repository.
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
$pkg = Join-Path $root 'projects\spare-construct\demo\transport\pkg'

# a small, fast release build: size-optimised, one codegen unit, link-time optimisation, no panic unwinding, no symbols
$env:CARGO_PROFILE_RELEASE_OPT_LEVEL = 's'
$env:CARGO_PROFILE_RELEASE_LTO = 'true'
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = '1'
$env:CARGO_PROFILE_RELEASE_PANIC = 'abort'
$env:CARGO_PROFILE_RELEASE_STRIP = 'symbols'
Push-Location $root
try {
  cargo build --offline -p construct-wasm --target wasm32-unknown-unknown --release
  if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
} finally { Pop-Location }

# the cached wasm-bindgen: never downloaded here
$bindgen = Get-ChildItem (Join-Path $env:LOCALAPPDATA '.wasm-pack\wasm-bindgen-*\wasm-bindgen.exe') -ErrorAction SilentlyContinue |
  Where-Object { (& $_.FullName --version) -match '0\.2\.118' } | Select-Object -First 1
if (-not $bindgen) { throw 'wasm-bindgen 0.2.118 is not in the wasm-pack cache (.wasm-pack under LOCALAPPDATA)' }

New-Item -ItemType Directory -Force $pkg | Out-Null
& $bindgen.FullName --target web --out-dir $pkg --out-name construct_wasm (Join-Path $target 'wasm32-unknown-unknown\release\construct_wasm.wasm')
if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen failed' }
Get-ChildItem $pkg | Select-Object Name, Length
