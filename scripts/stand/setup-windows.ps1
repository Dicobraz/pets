# Pets stand: shared Cargo build directory on a Dev Drive (H-01).
# One CARGO_TARGET_DIR for all Pets repos: shared dependencies (gpui and others) are built once.
# Sets the user variable only if the drive exists and is ReFS (Dev Drive); otherwise changes nothing.
# Without -Apply it only reports what it would do. The variable affects every repo built by this user.
# Usage: powershell -File scripts\stand\setup-windows.ps1 [-Drive D] [-Apply]
param(
  [string]$Drive = "D",
  [switch]$Apply
)

$vol = Get-Volume -DriveLetter $Drive -ErrorAction SilentlyContinue
if (-not $vol) { "Drive ${Drive}: not found - nothing to do (no Dev Drive)."; exit 0 }
if ($vol.FileSystemType -ne "ReFS") { "Drive ${Drive}: is $($vol.FileSystemType), not ReFS (Dev Drive) - nothing to do."; exit 0 }

$target = "${Drive}:\cargo-target"
$current = [Environment]::GetEnvironmentVariable("CARGO_TARGET_DIR", "User")
"Dev Drive ${Drive}: found. CARGO_TARGET_DIR now: '$current', proposed: '$target'."
if ($current -eq $target) { "Already set."; exit 0 }
if (-not $Apply) { "Run with -Apply to set it (setx, user scope). Old target\ folders can then be removed with clean.ps1."; exit 0 }
New-Item -ItemType Directory -Force $target | Out-Null
setx CARGO_TARGET_DIR $target | Out-Null
"Set CARGO_TARGET_DIR=$target for the user. Open a new terminal for it to take effect."
