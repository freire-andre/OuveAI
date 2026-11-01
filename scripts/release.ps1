# Gera o instalador do piloto e o SHA-256 dele (D4: sem assinatura, hash publicado).
# Uso: pwsh scripts/release.ps1        (na raiz do repositório)
# Não publica nada: o envio ao GitHub Releases é feito à mão, junto com o .sha256.
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
Write-Host "OuveAI ${version}: testes e build" -ForegroundColor Cyan

cargo test -p ouveai-core -p ouveai-audio --quiet
if ($LASTEXITCODE) { throw 'testes falharam' }
npx svelte-check --threshold error
if ($LASTEXITCODE) { throw 'svelte-check falhou' }
npx tauri build
if ($LASTEXITCODE) { throw 'build falhou' }

$setup = "target/release/bundle/nsis/OuveAI_${version}_x64-setup.exe"
if (-not (Test-Path $setup)) { throw "instalador não encontrado: $setup" }
$hash = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLower()
$name = Split-Path $setup -Leaf
"$hash  $name" | Set-Content -NoNewline -Encoding ascii "$setup.sha256"

$mb = [math]::Round((Get-Item $setup).Length / 1MB, 1)
Write-Host ""
Write-Host "Instalador: $setup ($mb MB)"
Write-Host "SHA-256:    $hash"
Write-Host "Arquivo:    $setup.sha256"
