# Pets stand: one-off %TEMP% cleanup (H-01). Lists, then deletes only with -Apply:
#  - folders and files whose names start with the given app prefixes (postman-, pets-, ...);
#  - abandoned tempfile folders/files (.tmp*) older than -Days.
# Usage: powershell -File scripts\stand\clean-temp.ps1 [-Prefixes postman,pets] [-Days 1] [-Apply]
param(
  [string[]]$Prefixes = @("postman", "pets"),
  [int]$Days = 1,
  [switch]$Apply
)

# `-File` passes "a,b" as one string: split lists here.
$Prefixes = $Prefixes | ForEach-Object { $_ -split ',' } | Where-Object { $_ }
# %TEMP% may be an 8.3 short path (C:\Users\A81F7~1.ANT\...): Remove-Item then fails - use the full path.
$temp = (Get-Item $env:TEMP).FullName
$cutoff = (Get-Date).AddDays(-$Days)
$items = Get-ChildItem $temp -Force -ErrorAction SilentlyContinue | Where-Object {
  $name = $_.Name
  ($Prefixes | Where-Object { $name -like "$_-*" }) -or ($name -like ".tmp*" -and $_.LastWriteTime -lt $cutoff)
}
$total = 0
foreach ($i in $items) {
  $size = if ($i.PSIsContainer) { (Get-ChildItem $i.FullName -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum } else { $i.Length }
  $total += $size
  "{0,-50} {1,10:N1} MB  {2:yyyy-MM-dd HH:mm}" -f $i.Name, ($size / 1MB), $i.LastWriteTime
}
"Total: {0} items, {1:N1} MB" -f @($items).Count, ($total / 1MB)
if (-not $Apply) { "Run with -Apply to delete."; exit 0 }
foreach ($i in $items) { Remove-Item -LiteralPath $i.FullName -Recurse -Force -ErrorAction SilentlyContinue }
"Deleted."
