'use strict';
// A no-install NSIS probe. Unique fixture names never select the installed client.
const fs = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { randomUUID } = require('node:crypto');
const { spawn, spawnSync } = require('node:child_process');
const assert = require('node:assert/strict');
const { getMakeNsisPath, getNsisPluginsPath } = require('app-builder-lib/out/toolsets/windows');
const root = path.join(__dirname, '..');
function quoted(value) { return '"' + value.replaceAll('$', '$$').replaceAll('"', '$\\"') + '"'; }
async function run() {
  if (process.platform !== 'win32') throw Error('Run the policy probe on Windows.');
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'palisade-nsis-policy-'));
  let fixture;
  try {
    const id = randomUUID();
    const filename = `Palisade-Policy-Fixture-${id}.exe`;
    const executable = path.join(directory, filename), source = path.join(directory, 'fixture.cs');
    await fs.writeFile(source, 'class Fixture { static void Main(string[] args) { System.Threading.Mutex gate=null; System.Threading.EventWaitHandle permit=null; if(args.Length==2) { bool created; gate=new System.Threading.Mutex(true,args[0],out created); if(!created) System.Environment.Exit(3); permit=new System.Threading.EventWaitHandle(true,System.Threading.EventResetMode.AutoReset,args[1]); } System.Console.WriteLine("ready"); string line; while ((line=System.Console.ReadLine())!=null) { if(line=="exit") break; System.Console.WriteLine("alive"); } if(permit!=null) permit.Dispose(); if(gate!=null) { gate.ReleaseMutex(); gate.Dispose(); } } }');
    const compiler = path.join(process.env.SystemRoot, 'Microsoft.NET/Framework64/v4.0.30319/csc.exe');
    const compiled = spawnSync(compiler, ['/nologo', '/target:exe', '/out:' + executable, source], { encoding: 'utf8', windowsHide: true });
    assert.equal(compiled.status, 0, compiled.stdout + compiled.stderr);
    const [nsis, plugins] = await Promise.all([getMakeNsisPath(), getNsisPluginsPath()]);
    const templates = path.join(path.dirname(require.resolve('app-builder-lib/package.json')), 'templates/nsis/include');
    const probe = path.join(directory, 'policy-probe.exe');
    const script = path.join(directory, 'policy-probe.nsi');
    await fs.writeFile(script, `Unicode true\nSilentInstall silent\nRequestExecutionLevel user\nName "Palisade no-install policy probe"\nOutFile ${quoted(probe)}\n!addincludedir ${quoted(templates)}\n!addplugindir /x86-unicode ${quoted(path.join(plugins, 'x86-unicode'))}\n!define APP_EXECUTABLE_FILENAME ${quoted(filename)}\n!define PALISADE_UPDATE_MUTEX_NAME ${quoted('Global\\Palisade-Policy-Probe-' + id)}\n!define PALISADE_UPDATE_PERMIT_PREFIX ${quoted('Global\\Palisade-Policy-Permit-' + id + '-')}\n!include ${quoted(path.join(root, 'assets/no-force-close.nsh'))}\nSection\n!insertmacro customCheckAppRunning\nSetErrorLevel 0\nSectionEnd\n`);
    const built = spawnSync(nsis.path, ['/V2', script], { env: { ...process.env, ...nsis.env }, encoding: 'utf8', windowsHide: true, timeout: 30000 });
    assert.equal(built.status, 0, built.stdout + built.stderr);
    const invoke = (args = []) => spawnSync(probe, ['/S', ...args], { encoding: 'utf8', windowsHide: true, timeout: 10000 });
    assert.equal(invoke().status, 0, 'Absent fixture must permit a no-op operation.');
    assert.equal(invoke(['--palisade-update=invalid']).status, 170, 'Forged permit must refuse.');
    assert.equal(invoke(['--palisade-update=' + 'a'.repeat(64)]).status, 170, 'Unknown permit must refuse.');
    fixture = spawn(executable, [], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
    const output = [];
    fixture.stdout.on('data', bytes => output.push(bytes.toString()));
    const deadline = Date.now() + 10000;
    while (!output.join('').includes('ready') && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 10));
    assert.ok(output.join('').includes('ready'));
    assert.equal(invoke().status, 170, 'A running client must refuse, without force-closing it.');
    fixture.stdin.write('ping\n');
    const aliveDeadline = Date.now() + 5000;
    while (!output.join('').includes('alive') && Date.now() < aliveDeadline) await new Promise(resolve => setTimeout(resolve, 10));
    assert.ok(output.join('').includes('alive'), 'The fixture retains its own process and input pipe.');
    const exited = new Promise((resolve, reject) => { fixture.once('exit', resolve); fixture.once('error', reject); });
    fixture.stdin.end('exit\n'); assert.equal(await exited, 0); fixture = null;
    assert.equal(invoke().status, 0);
    const nonce = 'b'.repeat(64);
    fixture = spawn(executable, ['Global\\Palisade-Policy-Probe-' + id, 'Global\\Palisade-Policy-Permit-' + id + '-' + nonce],
      { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('Permit owner did not start.')), 10000);
      fixture.stdout.once('data', bytes => { clearTimeout(timer); assert.match(bytes.toString(), /ready/); resolve(); });
      fixture.once('error', error => { clearTimeout(timer); reject(error); });
    });
    assert.equal(invoke().status, 170, 'Another installer cannot replace a retained operation.');
    assert.equal(invoke(['--palisade-update=' + nonce]).status, 170, 'A valid permit still cannot close a running client.');
    // The policy consumes a permit before checking the client; it cannot be replayed after refusal.
    assert.equal(invoke(['--palisade-update=' + nonce]).status, 170, 'Consumed permits cannot be replayed.');
    const ownerExited = new Promise(resolve => fixture.once('exit', resolve));
    fixture.stdin.end('exit\n'); assert.equal(await ownerExited, 0); fixture = null;
    assert.equal(invoke().status, 0);
    const ownerImage = path.join(directory, `Permit-Owner-${id}.exe`);
    await fs.copyFile(executable, ownerImage);
    const freshNonce = 'c'.repeat(64);
    fixture = spawn(ownerImage, ['Global\\Palisade-Policy-Probe-' + id, 'Global\\Palisade-Policy-Permit-' + id + '-' + freshNonce],
      { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(Error('Permit owner did not start.')), 10000);
      fixture.stdout.once('data', bytes => { clearTimeout(timer); assert.match(bytes.toString(), /ready/); resolve(); });
      fixture.once('error', error => { clearTimeout(timer); reject(error); });
    });
    assert.equal(invoke(['--palisade-update=' + freshNonce]).status, 0, 'A single fresh permit admits a no-op operation when no client is running.');
    assert.equal(invoke(['--palisade-update=' + freshNonce]).status, 170, 'A successful operation consumes its permit.');
    const freshExited = new Promise(resolve => fixture.once('exit', resolve));
    fixture.stdin.end('exit\n'); assert.equal(await freshExited, 0); fixture = null;
    console.log('No-install NSIS policy probe passed: absent/running clients, liveness, retained gates, forged and consumed permits. No installer or real Palisade client executed.');
  } finally {
    if (fixture && fixture.exitCode === null) { fixture.stdin.end('exit\n'); await new Promise(resolve => fixture.once('exit', resolve)); }
    await fs.rm(directory, { recursive: true, force: true });
  }
}
if (require.main === module) run().catch(error => { console.error(error.message); process.exitCode = 1; });
module.exports = { run };
