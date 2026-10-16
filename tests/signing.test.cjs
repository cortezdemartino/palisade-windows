'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs/promises');
const path=require('node:path');
const os=require('node:os');
const {generateKeyPairSync}=require('node:crypto');
const {spawnSync}=require('node:child_process');
test('release CLI signs and verifies both formats; missing credentials and tampered artifacts fail closed',async()=>{
 const dir=await fs.mkdtemp(path.join(os.tmpdir(),'palisade-signing-test-'));
 try {
  for(const folder of ['scripts','src/updates','assets','out']) await fs.mkdir(path.join(dir,folder),{recursive:true});
  for(const file of ['scripts/sign-release.cjs','scripts/verify-release.cjs','src/updates/release.cjs']) await fs.copyFile(path.join(__dirname,'..',file),path.join(dir,file));
  await fs.writeFile(path.join(dir,'package.json'),JSON.stringify({version:'0.2.1'}));
  const keys=generateKeyPairSync('ed25519');
  const keyfile=path.join(dir,'private.pem');
  await fs.writeFile(keyfile,keys.privateKey.export({type:'pkcs8',format:'pem'}),{mode:0o600});
  await fs.writeFile(path.join(dir,'assets/update-public-key.pem'),keys.publicKey.export({type:'spki',format:'pem'}));
  const installer=path.join(dir,'out/Palisade-Setup-0.2.1-x64.exe'); await fs.writeFile(installer,'fixture installer');
  const invoke=(script,args=[],credential=keyfile)=>spawnSync(process.execPath,[path.join(dir,'scripts',script),...args],{cwd:dir,encoding:'utf8',env:{...process.env,PALISADE_UPDATE_PRIVATE_KEY_FILE:credential}});
  assert.notEqual(invoke('sign-release.cjs',[installer,'out'],'').status,0);
  const signed=invoke('sign-release.cjs',[installer,'out']); assert.equal(signed.status,0,signed.stderr);
  const verified=invoke('verify-release.cjs',['out']); assert.equal(verified.status,0,verified.stderr);
  await fs.appendFile(installer,'tampering'); assert.notEqual(invoke('verify-release.cjs',['out']).status,0);
  await fs.writeFile(path.join(dir,'assets/update-public-key.pem'),generateKeyPairSync('ed25519').publicKey.export({type:'spki',format:'pem'}));
  assert.notEqual(invoke('sign-release.cjs',[installer,'out']).status,0);
 } finally {await fs.rm(dir,{recursive:true,force:true});}
});
