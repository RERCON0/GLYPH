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
    Copy-Item README.md, README.ru.md, SECURITY.md, SECURITY.ru.md, LICENSE dist/ -Force
    Copy-Item fonts/OFL-notice.txt dist/FONT-LICENSE.txt -Force
    Copy-Item fonts/OFL.txt dist/OFL.txt -Force
    python scripts/check_artifact.py GLYPH.exe
    if ($LASTEXITCODE) { throw 'artifact check failed' }
    python scripts/check_artifact.py dist/GLYPH.exe
    if ($LASTEXITCODE) { throw 'packaged artifact check failed' }
    $hash = (Get-FileHash dist/GLYPH.exe -Algorithm SHA256).Hash
    "$hash  GLYPH.exe" | Set-Content -Encoding ascii dist/SHA256SUMS.txt
    $metadata = cargo metadata --no-deps --locked --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'cargo metadata failed' }
    $version = ($metadata.packages | Where-Object name -eq 'glyph').version
    $commit = (git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -or $commit -notmatch '^[0-9a-f]{40}$') { throw 'Cannot determine source revision' }
    @("GLYPH $version", "Source: https://github.com/RERCON0/GLYPH/tree/$commit",
        'Windows x64; statically linked MSVC CRT. Extract and run GLYPH.exe.') |
        Set-Content -Encoding ascii dist/SOURCE.txt
    $payload = @('GLYPH.exe', 'README.md', 'README.ru.md', 'SECURITY.md', 'SECURITY.ru.md',
        'LICENSE', 'FONT-LICENSE.txt', 'OFL.txt', 'SHA256SUMS.txt', 'SOURCE.txt') |
        ForEach-Object { Join-Path 'dist' $_ }
    $archive = "dist/GLYPH-$version-windows-x64.zip"
    Compress-Archive -LiteralPath $payload -DestinationPath $archive -Force
    $archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash
    "$archiveHash  $(Split-Path -Leaf $archive)" | Set-Content -Encoding ascii "$archive.sha256"
    Get-FileHash dist/GLYPH.exe -Algorithm SHA256
} finally {
    Pop-Location
}
