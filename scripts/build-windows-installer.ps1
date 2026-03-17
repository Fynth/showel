param(
    [ValidateSet("exe", "msi")]
    [string]$PackageType = "exe"
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command dx -ErrorAction SilentlyContinue)) {
    Write-Error "dioxus-cli (dx) is not installed. Install it first: cargo install dioxus-cli --locked"
}

Write-Host "Building Windows installer for Showel..."
dx bundle --platform desktop --release --features bundle --package-types $PackageType
