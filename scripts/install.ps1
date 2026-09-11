$ErrorActionPreference = "Stop"

$Repo = "dannymaaz/SwitchCraft"
$AppName = "SwitchCraft"

Write-Host ""
Write-Host "  ⚡ $AppName Installer" -ForegroundColor Yellow
Write-Host "  ─────────────────────────"
Write-Host ""

# Get latest release
$ReleaseUrl = "https://api.github.com/repos/$Repo/releases/latest"
Write-Host "  Fetching latest release..."

try {
    $Release = Invoke-RestMethod -Uri $ReleaseUrl -Method Get
} catch {
    Write-Host "  Failed to fetch release info. Check your internet connection." -ForegroundColor Red
    exit 1
}

# Find MSI asset
$Asset = $Release.assets | Where-Object { $_.name -like "*.msi" } | Select-Object -First 1

if (-not $Asset) {
    Write-Host "  No MSI installer found in the latest release." -ForegroundColor Red
    Write-Host "  Visit https://github.com/$Repo/releases for manual download."
    exit 1
}

$DownloadUrl = $Asset.browser_download_url
$FileName = $Asset.name
$TempPath = Join-Path $env:TEMP $FileName

Write-Host "  Downloading $FileName..."
Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempPath

Write-Host "  Installing..."
Start-Process msiexec.exe -ArgumentList "/i `"$TempPath`" /qn" -Wait -NoNewWindow

Write-Host ""
Write-Host "  Done. $AppName is installed!" -ForegroundColor Green
Write-Host "  You can find it in your Start Menu." 
Write-Host ""

# Cleanup
Remove-Item $TempPath -Force -ErrorAction SilentlyContinue
