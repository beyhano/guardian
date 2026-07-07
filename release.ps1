param(
    [string]$Version,
    [switch]$SkipBuild,
    [switch]$Help
)

# Guardian Release Script
# Kullanım:
#   .\release.ps1                           # interactive
#   .\release.ps1 -Version 0.2.0            # belirtilen versiyon
#   .\release.ps1 -Version 0.2.0 -SkipBuild # build'i atla, GitHub yapsin

$ErrorActionPreference = "Stop"

if ($Help) {
    Write-Host @"
GUARDIAN Release Script

Kullanim:
  .\release.ps1                        - Mevcut surumu goster, versiyon sor
  .\release.ps1 -Version 1.2.3         - Belirtilen surumu release et
  .\release.ps1 -Version 1.2.3 -SkipBuild - Tag at, build'i GitHub'a birak

Islem sirasi:
  1. Versiyon kontrolu
  2. Local build (pnpm build + cargo check) -SkipBuild ile atlanabilir
  3. tauri.conf.json + package.json + Cargo.toml versiyon guncelleme
  4. Git commit + tag + push
  5. GitHub Actions build'i otomatik tetiklenir
"@
    exit 0
}

# --- mevcut versiyonu oku ---
$tauriRaw = Get-Content "src-tauri\tauri.conf.json" -Raw
$current = ($tauriRaw | Select-String '"version":\s*"([^"]+)"').Matches.Groups[1].Value

# --- versiyon sec / dogrula ---
if (-not $Version) {
    Write-Host "Mevcut surum: v$current"
    $v = Read-Host "Yeni surum (ENTER = $current)"
    if ($v) { $Version = $v } else { $Version = $current }
}

if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    Write-Host "HATA: '$Version' gecerli degil (X.Y.Z)" -ForegroundColor Red
    exit 1
}

# --- degisiklik kontrolu ---
$status = git status --porcelain
if ($status) {
    Write-Host "`n! Commit edilmemis degisiklikler var:" -ForegroundColor Yellow
    $status
    $yes = Read-Host "`nHepsini commit'leyip release'e dahil edelim mi? (e/H)"
    if ($yes -eq "e") {
        $msg = Read-Host "Commit mesaji (ENTER = v$Version hazirlik)"
        if (-not $msg) { $msg = "v$Version hazirlik" }
        git add -A
        git commit -m $msg
        git push origin main
    } else {
        Write-Host "Iptal." -ForegroundColor Red
        exit 1
    }
}

# --- build kontrolu ---
if (-not $SkipBuild) {
    Write-Host "`n==> pnpm build ..." -ForegroundColor Cyan
    pnpm build
    if ($LASTEXITCODE -ne 0) { throw "Build hatasi" }

    Write-Host "==> cargo check ..." -ForegroundColor Cyan
    cargo check --manifest-path src-tauri/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw "cargo check hatasi" }

    Write-Host "==> Build basarili`n" -ForegroundColor Green
} else {
    Write-Host "`n==> Build atlaniyor (GitHub Actions build alacak)" -ForegroundColor Yellow
}

# --- CHANGELOG uyarisi ---
$changelog = Get-Content "CHANGELOG.md" -Raw
if ($changelog -notmatch "## v$Version") {
    Write-Host "! CHANGELOG.md'de '## v$Version' bolumu yok" -ForegroundColor Yellow
    Write-Host "  Ctrl+C ile iptal edip guncelle veya ENTER'a bas devam et" -ForegroundColor Yellow
    Read-Host
}

# --- versiyon guncelleme ---
$vOld = "`"$current`""
$vNew = "`"$Version`""

$paths = @("src-tauri\tauri.conf.json", "package.json")
foreach ($p in $paths) {
    $c = Get-Content $p -Raw
    $c = $c -replace '"version": ' + $vOld, '"version": ' + $vNew
    [System.IO.File]::WriteAllText((Resolve-Path $p).Path, $c)
}

$cargo = Get-Content "src-tauri\Cargo.toml" -Raw
$cargo = $cargo -replace 'version = "' + $current + '"', 'version = "' + $Version + '"'
[System.IO.File]::WriteAllText((Resolve-Path "src-tauri\Cargo.toml").Path, $cargo)

Write-Host "Versiyon: v$current -> v$Version (tauri.conf.json + package.json + Cargo.toml)" -ForegroundColor Cyan

# --- commit + tag + push ---
$msg = "chore: v$Version release"
git add -A
git commit -m $msg
git tag "v$Version"
git push origin main
git push origin "v$Version"

Write-Host "`n🚀 v$Version release tetiklendi!" -ForegroundColor Green
Write-Host "   https://github.com/beyhano/guardian/actions" -ForegroundColor Cyan
Write-Host "   https://github.com/beyhano/guardian/releases" -ForegroundColor Cyan
