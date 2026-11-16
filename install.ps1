# ScriptVox installer for Windows. Never needs admin rights, never changes PATH, the registry or a profile.
#   powershell -ExecutionPolicy Bypass -File install.ps1 [-From scriptvox-<v>-x86_64-pc-windows-gnullvm.zip]
#   -Dest <folder>     where scriptvox.exe goes (or SCRIPTVOX_BIN_DIR; default %LOCALAPPDATA%\scriptvox\bin)
#   -BaseUrl <address> the folder address that holds the zip and its .sha256 file (or SCRIPTVOX_BASE_URL):
#                      a release mirror, or a local test server
param([string]$From = "", [string]$Version = "0.1.0", [string]$Dest = "", [string]$BaseUrl = "")
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"  # the progress bar of Windows PowerShell 5.1 makes a download very slow
$name = "scriptvox-$Version-x86_64-pc-windows-gnullvm"
if (-not $Dest) { $Dest = $env:SCRIPTVOX_BIN_DIR }
if (-not $Dest) { $Dest = Join-Path $env:LOCALAPPDATA "scriptvox\bin" }
if (-not $BaseUrl) { $BaseUrl = $env:SCRIPTVOX_BASE_URL }
if (-not $BaseUrl) { $BaseUrl = "https://github.com/scriptvox/scriptvox/releases/download/v$Version" }
$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory -Force -Path $tmp, $Dest | Out-Null
try {
  $zip = Join-Path $tmp "$name.zip"
  $sum = "$zip.sha256"
  if ($From) {
    if (-not (Test-Path $From)) { throw "scriptvox: no file '$From'. Use: install.ps1 -From <zip>" }
    Copy-Item $From $zip
    if (Test-Path "$From.sha256") { Copy-Item "$From.sha256" $sum }
  } else {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $url = $BaseUrl.TrimEnd("/") + "/$name.zip"
    Write-Output "scriptvox: downloading $url"
    try {
      Invoke-WebRequest $url -OutFile $zip -UseBasicParsing
      # A release always has the checksum file: without it nothing is installed.
      Invoke-WebRequest "$url.sha256" -OutFile $sum -UseBasicParsing
    } catch { throw "scriptvox: download failed: $url (and its .sha256). Check the address (-BaseUrl, -Version) and the network, nothing installed. $_" }
  }
  if (Test-Path $sum) {
    $want = ((Get-Content $sum -Raw).Trim() -split "\s+")[0]
    if ((Get-FileHash $zip -Algorithm SHA256).Hash.ToLower() -ne $want.ToLower()) { throw "scriptvox: checksum mismatch, nothing installed" }
    Write-Output "scriptvox: sha256 ok"
  }
  Expand-Archive $zip -DestinationPath $tmp -Force
  Copy-Item (Join-Path $tmp "$name\scriptvox.exe") $Dest -Force
} finally { Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue }
$exe = Join-Path $Dest "scriptvox.exe"
Write-Output "scriptvox installed: $exe"
Write-Output "Add $Dest to your user PATH if you want (this installer does not change it), or call it by its full path."
& $exe --version --json
