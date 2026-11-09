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
  [Console]::Out.WriteLine('observer-ready')
  $items = ConvertFrom-Json -InputObject ([Console]::In.ReadLine())
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
async function prepareProcessWatcher(binary) {
  const observer = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-EncodedCommand',
    Buffer.from(script, 'utf16le').toString('base64')], {
    windowsHide: true, shell: false, stdio: ['pipe', 'pipe', 'pipe'],
    env: { ...process.env, PALISADE_TEST_PROCESS_IMAGE: binary },
  });
  observer.stdin.on('error', () => {});
  let errors = ''; observer.stderr.on('data', bytes => { errors = (errors + bytes).slice(-4096); });
  const lines = createInterface({ input: observer.stdout });
  const result = [];
  let streamFailed = false;
  let retainedResolve, retainedReject;
  const retained = new Promise((resolve, reject) => { retainedResolve = resolve; retainedReject = reject; });
  void retained.catch(() => {});
  const ready = new Promise((resolve, reject) => {
    lines.on('line', line => {
      result.push(line);
      if (line === 'observer-ready') resolve();
      if (line === 'handles-retained') retainedResolve();
    });
    observer.once('error', reject);
    observer.once('error', retainedReject);
    const streamError = error => { streamFailed = true; reject(error); retainedReject(error); };
    observer.stdout.once('error', streamError);
    lines.once('error', streamError);
    observer.once('close', code => {
      const error = Error('Process observer ended: ' + code + ' ' + errors);
      if (!result.includes('observer-ready')) reject(error);
      if (!result.includes('handles-retained')) retainedReject(error);
    });
  });
  // Only this read-only observer is reaped on harness failure. It neither
  // signals nor terminates any observed process, and has no Assist authority.
  let timer = setTimeout(() => observer.kill(), 12_000);
  function deadline(milliseconds) {
    clearTimeout(timer);
    timer = setTimeout(() => observer.kill(), milliseconds);
  }
  // Exit can precede stdout drain. Validate the terminal JSON only after close.
  const closed = once(observer, 'close');
  closed.catch(() => {}); // Ready failures also own and report a failed launch.
  try { await ready; } catch (error) { clearTimeout(timer); observer.kill(); throw error; }
  deadline(12_000); // Retention setup has its own bound after cold shell startup.
  let registered = false;
  return {
    async retain(entries) {
      assert.equal(registered, false, 'Process observer already registered');
      registered = true;
      observer.stdin.write(JSON.stringify(entries) + '\n');
      await retained;
    },
    async finish(timeout = 4000) {
      assert.equal(registered, true, 'Process observer not registered');
      await retained;
      // The native assertion remains four seconds; allow only output/observer
      // teardown beyond that, measured separately from shell startup.
      deadline(6000);
      observer.stdin.end(String(timeout) + '\n');
      const [code] = await closed;
      clearTimeout(timer);
      assert.equal(code, 0, errors);
      assert.equal(streamFailed, false, 'Process observer output failed');
      assert.equal(result.length, 3, 'Unexpected observer protocol: ' + result.join('\n'));
      assert.deepEqual(result.slice(0, 2), ['observer-ready', 'handles-retained']);
      return JSON.parse(result[2]);
    },
    close() { clearTimeout(timer); observer.kill(); lines.close(); },
  };
}
module.exports = { prepareProcessWatcher };
