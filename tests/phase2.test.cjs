'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const { mkdtemp, rm, readFile, writeFile, readdir } = require('node:fs/promises');
const path = require('node:path');
const os = require('node:os');
const { generateKeyPairSync, createHash, sign } = require('node:crypto');
const { normalisePayload, signedBytes, verifyEnvelope, compareVersions, verifyDownloadedFile, fetchRelease, RELEASES_API } = require('../src/updates/release.cjs');
const { downloadInstaller } = require('../src/updates/downloader.cjs');
const { loadSettings, saveSettings } = require('../src/settings.cjs');
const bytes = Buffer.from('installer');
const manifest = normalisePayload({ schema: 2, channel: 'stable', version: '0.2.1', filename: 'Palisade-Setup-0.2.1-x64.exe', size: bytes.length, sha512: createHash('sha512').update(bytes).digest('base64'), sha256: createHash('sha256').update(bytes).digest('hex'), publishedAt: '2026-10-15T10:00:00.000Z' });
const { privateKey, publicKey } = generateKeyPairSync('ed25519');
const key = publicKey.export({ type: 'spki', format: 'pem' });
test('semantic precedence includes prereleases, numeric identifiers and build metadata', () => {
  const ordered = ['1.0.0-alpha', '1.0.0-alpha.1', '1.0.0-alpha.2', '1.0.0-alpha.10', '1.0.0-beta', '1.0.0-rc.1', '1.0.0', '1.0.1'];
  for (let i=1;i<ordered.length;i++) assert.equal(compareVersions(ordered[i], ordered[i-1]), 1);
  assert.equal(compareVersions('1.0.0+build.1','1.0.0+build.2'),0);
  assert.throws(() => compareVersions('1.0.0-01','1.0.0'));
});
test('v2 authenticates SHA-256 and refuses legacy envelope fallback', async () => {
  const envelope = { payload: manifest, signature: sign(null, signedBytes(manifest), privateKey).toString('base64') };
  assert.deepEqual(verifyEnvelope(envelope,key),manifest);
  assert.throws(() => verifyEnvelope({ ...envelope, payload: { ...manifest, sha256: '0'.repeat(64) } },key));
  assert.throws(() => normalisePayload({ ...manifest, publishedAt: '2026-02-30T00:00:00.000Z' }));
  const legacy = { ...manifest, schema: 1 }; delete legacy.sha256;
  const legacyEnvelope = { payload: legacy, signature: sign(null, signedBytes(legacy),privateKey).toString('base64') };
  const response = (value,url) => ({ ok:true, url, headers:new Map(), body:(async function*(){yield Buffer.from(JSON.stringify(value));})() });
  await assert.rejects(fetchRelease(async (url) => response(url === RELEASES_API ? { draft:false, prerelease:false, tag_name:'v0.2.1', assets:[{name:'release.json'},{name:manifest.filename}] } : legacyEnvelope,url),key,'0.2.0'), /rotation-aware/);
});
test('SHA-256 mismatch is refused independently of a correct SHA-512', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(),'palisade-test-'));
  try {
    const file=path.join(dir,manifest.filename); await writeFile(file,bytes);
    assert.equal(await verifyDownloadedFile(file,{...manifest,sha256:'0'.repeat(64)}),false);
    await rm(file);
    await assert.rejects(downloadInstaller({...manifest,sha256:'0'.repeat(64)},dir,async()=>({ok:true,status:200,body:(async function*(){yield bytes;})()})));
    assert.deepEqual(await readdir(dir),[]);
  } finally { await rm(dir,{recursive:true,force:true}); }
});
test('network errors, interrupted and oversized bodies clean up only files created by this download', async () => {
  const dir=await mkdtemp(path.join(os.tmpdir(),'palisade-test-'));
  try {
    await assert.rejects(downloadInstaller(manifest,dir,async()=>{throw new Error('offline');}));
    for (const body of [(async function*(){yield bytes.subarray(0,2);throw new Error('disconnected');})(), (async function*(){yield Buffer.alloc(50);})()]) {
      await assert.rejects(downloadInstaller(manifest,dir,async()=>({ok:true,status:200,body})));
      assert.deepEqual(await readdir(dir),[]);
    }
    const file=path.join(dir,manifest.filename); await writeFile(file,'existing');
    await assert.rejects(downloadInstaller(manifest,dir,async()=>({ok:true,status:200,body:(async function*(){yield bytes;})()})));
    assert.equal(await readFile(file,'utf8'),'existing');
  } finally { await rm(dir,{recursive:true,force:true}); }
});
test('desktop settings survive restart without changing session or user data', async () => {
  const dir=await mkdtemp(path.join(os.tmpdir(),'palisade-test-'));
  try {
    const profile=path.join(dir,'matrix-session'); await writeFile(profile,'session fixture');
    assert.equal(loadSettings(dir).keepRunningInTray,false);
    saveSettings(dir,{keepRunningInTray:true}); assert.equal(loadSettings(dir).keepRunningInTray,true);
    await writeFile(path.join(dir,'desktop-settings.json'),'{broken');
    assert.equal(loadSettings(dir).keepRunningInTray,false);
    assert.equal(await readFile(profile,'utf8'),'session fixture');
    const pkg=require('../package.json');
    assert.equal(pkg.name,'@palisade/desktop'); assert.equal(pkg.build.appId,'org.onyxops.palisade');
    assert.equal(pkg.build.productName,'Palisade'); assert.equal(pkg.build.nsis.deleteAppDataOnUninstall,false);
    const main=await readFile(path.join(__dirname,'../src/main.cjs'),'utf8');
    assert.match(main,/persist:palisade-desktop/);
    assert.doesNotMatch(main,/clearStorageData|clearAuthCache|setPath\(['"]userData/);
  } finally { await rm(dir,{recursive:true,force:true}); }
});
