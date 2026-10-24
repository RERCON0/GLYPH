$ErrorActionPreference = 'Stop'
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE) { throw 'rustfmt failed' }
    cargo clippy --all-targets --all-features --locked -- -D warnings
    if ($LASTEXITCODE) { throw 'clippy failed' }
    cargo test --all-targets --all-features --locked
    if ($LASTEXITCODE) { throw 'tests failed' }
    cargo build --release --locked --bin glyph
    if ($LASTEXITCODE) { throw 'release build failed' }
    New-Item -ItemType Directory -Force dist | Out-Null
    Copy-Item target/release/glyph.exe GLYPH.exe -Force
    Copy-Item target/release/glyph.exe dist/GLYPH.exe -Force
    Copy-Item README.md, README.ru.md, SECURITY.md, LICENSE dist/ -Force
    Copy-Item fonts/OFL-notice.txt dist/FONT-LICENSE.txt -Force
    Copy-Item fonts/OFL.txt dist/OFL.txt -Force
    python scripts/check_artifact.py GLYPH.exe
    if ($LASTEXITCODE) { throw 'artifact check failed' }
    python scripts/check_artifact.py dist/GLYPH.exe
    if ($LASTEXITCODE) { throw 'packaged artifact check failed' }
    $hash = (Get-FileHash dist/GLYPH.exe -Algorithm SHA256).Hash
    "$hash  GLYPH.exe" | Set-Content -Encoding ascii dist/SHA256SUMS.txt
    Get-FileHash dist/GLYPH.exe -Algorithm SHA256
} finally {
    Pop-Location
}
