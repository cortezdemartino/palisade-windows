param(
  [int]$ParentId,
  [string]$Installer,
  [string]$Executable,
  [string]$Receipt,
  [string]$Version,
  [string]$Sha256,
  [string]$Handshake
)
$ErrorActionPreference = 'Stop'
$code = -1
$stage = 'parent-exit'
try {
  Set-Content -LiteralPath $Handshake -Value 'ready' -Encoding ascii
  $parent = Get-Process -Id $ParentId -ErrorAction SilentlyContinue
  if ($parent) { $parent.WaitForExit() }
  $stage = 'checksum'
  $stream = [IO.File]::OpenRead($Installer)
  $hash = [Security.Cryptography.SHA256]::Create()
  try { $actualHash = [BitConverter]::ToString($hash.ComputeHash($stream)).Replace('-', '').ToLowerInvariant() }
  finally { $stream.Dispose(); $hash.Dispose() }
  if ($actualHash -cne $Sha256) {
    $stage = 'checksum-mismatch'
    throw 'Installer changed before execution.'
  }
  $stage = 'installer'
  $process = Start-Process -FilePath $Installer -ArgumentList '/S', '--force-run' -PassThru -Wait
  $code = $process.ExitCode
} catch { $code = -1 }
@{ version = $Version; exitCode = $code; stage = $stage } | ConvertTo-Json | Set-Content -LiteralPath $Receipt -Encoding utf8
if ($code -ne 0) {
  Start-Process -FilePath $Executable -WindowStyle Hidden
}
