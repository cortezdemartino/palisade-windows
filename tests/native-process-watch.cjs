'use strict';
// Test-only observer. Retain original Windows process handles BEFORE allowing
// the fixture parent to exit; recycled PIDs can never satisfy this assertion.
const { spawn } = require('node:child_process');
const { createInterface } = require('node:readline');
const { once } = require('node:events');
const assert = require('node:assert/strict');
const script = String.raw`
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$owned = @()
try {
  $items = ConvertFrom-Json -InputObject $env:PALISADE_TEST_PROCESS_HANDLES
  if ($items.Count -lt 1 -or $items.Count -gt 3) { throw 'Unexpected process count' }
  foreach ($item in $items) {
    if ($item.pid -le 0 -or $item.role -notin @('monitor','coordinator','holder')) { throw 'Invalid fixture identity' }
    $process = [Diagnostics.Process]::GetProcessById([int]$item.pid)
    [void]$process.SafeHandle
    if ($process.MainModule.FileName -ine $env:PALISADE_TEST_PROCESS_IMAGE) { throw 'Wrong fixture image' }
    $owned += [pscustomobject]@{ process=$process; role=$item.role; pid=[int]$item.pid; created=[string]$process.StartTime.ToUniversalTime().Ticks }
  }
  [Console]::Out.WriteLine('handles-retained')
  $timeoutText = [Console]::In.ReadLine()
  $timeout = 0
  if (-not [int]::TryParse($timeoutText, [ref]$timeout) -or $timeout -lt 1 -or $timeout -gt 4000) { throw 'Invalid wait' }
  $clock = [Diagnostics.Stopwatch]::StartNew()
  $result = @()
  foreach ($item in $owned) {
    $remaining = [Math]::Max(0, $timeout - [int]$clock.ElapsedMilliseconds)
    $exited = $item.process.WaitForExit($remaining)
    $code = $null
    if ($exited) { $code = $item.process.ExitCode }
    $result += [pscustomobject]@{role=$item.role;pid=$item.pid;created=$item.created;exited=$exited;code=$code}
  }
  [Console]::Out.WriteLine((ConvertTo-Json -InputObject $result -Compress))
  [GC]::KeepAlive($owned)
} finally {
  foreach ($item in $owned) { $item.process.Dispose() }
}
`;
async function watchProcesses(binary, entries) {
  const observer = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand',
    Buffer.from(script, 'utf16le').toString('base64')], {
    windowsHide: true, shell: false, stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, PALISADE_TEST_PROCESS_HANDLES: JSON.stringify(entries),
      PALISADE_TEST_PROCESS_IMAGE: binary },
  });
  observer.stdin.on('error', () => {});
  let errors = ''; observer.stderr.on('data', bytes => { errors = (errors + bytes).slice(-4096); });
  const lines = createInterface({ input: observer.stdout });
  const result = [];
  let streamFailed = false;
  const ready = new Promise((resolve, reject) => {
    lines.on('line', line => { result.push(line); if (line === 'handles-retained') resolve(); });
    observer.once('error', reject);
    const streamError = error => { streamFailed = true; reject(error); };
    observer.stdout.once('error', streamError);
    lines.once('error', streamError);
    observer.once('exit', code => { if (!result.includes('handles-retained')) reject(Error('Process observer ended: ' + code + ' ' + errors)); });
  });
  // Only this read-only observer is reaped on harness failure. It neither
  // signals nor terminates any observed process, and has no Assist authority.
  const timer = setTimeout(() => observer.kill(), 12_000);
  // Exit can precede stdout drain. Validate the terminal JSON only after close.
  const closed = once(observer, 'close');
  closed.catch(() => {}); // Ready failures also own and report a failed launch.
  try { await ready; } catch (error) { clearTimeout(timer); observer.kill(); throw error; }
  return {
    async finish(timeout = 4000) {
      observer.stdin.end(String(timeout) + '\n');
      const [code] = await closed;
      clearTimeout(timer);
      assert.equal(code, 0, errors);
      assert.equal(streamFailed, false, 'Process observer output failed');
      assert.equal(result.length, 2, 'Unexpected observer protocol: ' + result.join('\n'));
      return JSON.parse(result[1]);
    },
    close() { clearTimeout(timer); observer.kill(); lines.close(); },
  };
}
module.exports = { watchProcesses };
