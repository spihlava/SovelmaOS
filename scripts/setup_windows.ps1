# SovelmaOS Development Environment Setup Script
# Run this from the repository root: ./scripts/setup_windows.ps1

Write-Host "Setting up SovelmaOS environment..." -ForegroundColor Cyan

# 1. Check Rust
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    Write-Host "Rust is installed." -ForegroundColor Green
    rustc --version
}
else {
    Write-Error "Rust is NOT installed. Please install from https://rustup.rs/"
    exit 1
}

# 2. Install Targets
Write-Host "Installing Rust targets..." -ForegroundColor Yellow
rustup target add x86_64-unknown-none
rustup target add riscv32imac-unknown-none-elf

# 3. Install Tools
Write-Host "Installing build tools..." -ForegroundColor Yellow

# cargo-binutils (for objcopy, readobj)
if (-not (Get-Command cargo-objcopy -ErrorAction SilentlyContinue)) {
    rustup component add llvm-tools-preview
    cargo install cargo-binutils
}

# espflash (for flashing ESP32)
if (-not (Get-Command espflash -ErrorAction SilentlyContinue)) {
    cargo install espflash
}

# 4. Check QEMU
if (Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue) {
    Write-Host "QEMU is installed." -ForegroundColor Green
}
else {
    Write-Warning "QEMU not found in PATH. Please install QEMU for Windows and add to PATH."
    Write-Warning "Download: https://www.qemu.org/download/#windows"
}
