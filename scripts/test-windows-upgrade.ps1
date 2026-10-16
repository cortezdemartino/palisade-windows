param([switch]$Execute)
$ErrorActionPreference = 'Stop'
if (-not $Execute) { throw 'Run with -Execute after building the two isolated installers described in docs/UPGRADE-TEST.md.' }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\out\upgrade-test'))
$install = [IO.Path]::GetFullPath((Join-Path $root 'installed'))
if ([IO.Path]::GetDirectoryName($install) -cne $root) { throw 'Invalid test installation path.' }
$existing = Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\* -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -like 'PalisadePhase2Test*' }
if ($existing -or (Test-Path -LiteralPath $install)) { throw 'An isolated test installation already exists. Inspect and remove it before repeating.' }
$baseline = Join-Path $root 'baseline\Palisade-Setup-0.2.0-x64.exe'
$candidate = Join-Path $root 'candidate\Palisade-Setup-0.2.1-x64.exe'
foreach ($file in @($baseline,$candidate)) { if (-not (Test-Path -LiteralPath $file)) { throw "Missing installer: $file" } }
$profile = [IO.Path]::GetFullPath((Join-Path $env:APPDATA 'PalisadePhase2Test'))
if (Test-Path -LiteralPath $profile) { throw 'Test profile already exists.' }
$partition = Join-Path $profile 'Partitions\palisade-desktop'
New-Item -ItemType Directory -Path $partition -Force | Out-Null
$fixture = Join-Path $partition 'phase2-preservation-fixture.txt'
Set-Content -LiteralPath $fixture -Value 'phase2-profile-fixture' -Encoding ascii
try {
 foreach ($stage in @(@{file=$baseline;version='0.2.0'},@{file=$candidate;version='0.2.1'})) {
  $process = Start-Process -FilePath $stage.file -ArgumentList "/S /D=$install" -WindowStyle Hidden -PassThru -Wait
  if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
  $exe = Join-Path $install 'PalisadePhase2Test.exe'
  $actual = (Get-Item -LiteralPath $exe).VersionInfo.ProductVersion
  if ($actual -notin @($stage.version, "$($stage.version).0")) { throw "Expected $($stage.version), found $actual" }
  if ((Get-Content -LiteralPath $fixture -Raw).Trim() -ne 'phase2-profile-fixture') { throw 'Profile fixture changed.' }
  Write-Output "PASS: installed $actual at the same test path; profile fixture preserved."
 }
} finally {
 $uninstaller = Join-Path $install 'Uninstall PalisadePhase2Test.exe'
 if (Test-Path -LiteralPath $uninstaller) {
  $process = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
  if ($process.ExitCode -ne 0) { throw 'Test uninstall failed.' }
 }
 if ([IO.Path]::GetDirectoryName($profile) -cne [IO.Path]::GetFullPath($env:APPDATA)) { throw 'Invalid test profile cleanup path.' }
 Remove-Item -LiteralPath $profile -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Output 'PASS: isolated NSIS upgrade and uninstall. Application login, relaunch and GitHub distribution are not covered.'
