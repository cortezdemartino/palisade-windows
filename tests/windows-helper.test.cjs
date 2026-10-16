'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs/promises');
const path=require('node:path');
const os=require('node:os');
const {spawn,spawnSync}=require('node:child_process');
const {createHash}=require('node:crypto');
test('Windows installer helper waits for parent, passes NSIS flags, and rejects changed bytes',{skip:process.platform!=='win32',timeout:60000},async()=>{
 const dir=await fs.mkdtemp(path.join(os.tmpdir(),'palisade-helper-'));
 const powershell=path.join(process.env.SystemRoot,'System32/WindowsPowerShell/v1.0/powershell.exe');
 let parent;
 try{
  const compiler=path.join(dir,'compile.ps1'),exe=path.join(dir,'fixture.exe');
  await fs.writeFile(compiler,`param([string]$Destination)\n$ErrorActionPreference='Stop'\nAdd-Type -TypeDefinition 'public class Program { public static int Main(string[] args) { return args.Length == 2 && args[0] == "/S" && args[1] == "--force-run" ? (System.Environment.GetEnvironmentVariable("PALISADE_TEST_INSTALL_FAIL") == "1" ? 17 : 0) : 11; } }' -OutputAssembly $Destination -OutputType WindowsApplication\n`);
  const compiled=spawnSync(powershell,['-NoProfile','-NonInteractive','-File',compiler,'-Destination',exe],{encoding:'utf8',windowsHide:true});
  assert.equal(compiled.status,0,compiled.stderr);
  const digest=createHash('sha256').update(await fs.readFile(exe)).digest('hex');
  for(const scenario of ["success","tampered","installer-failure"]){
   const tampered=scenario==="tampered";
   parent=spawn(process.execPath,['-e','setTimeout(()=>{},20000)'],{windowsHide:true,stdio:'ignore'});
   const receipt=path.join(dir,`result-${scenario}.json`),handshake=path.join(dir,`ready-${scenario}`);
   const helper=spawn(powershell,['-NoProfile','-NonInteractive','-File',path.join(__dirname,'../src/updates/install.ps1'),'-ParentId',String(parent.pid),'-Installer',exe,'-Executable',exe,'-Receipt',receipt,'-Version','0.2.1','-Sha256',tampered?'0'.repeat(64):digest,'-Handshake',handshake],{windowsHide:true,stdio:'ignore',env:{...process.env,PALISADE_TEST_INSTALL_FAIL:scenario==='installer-failure'?'1':'0'}});
   const finished=new Promise((resolve,reject)=>{helper.on('error',reject);helper.on('exit',resolve);});
   let ready=false;
   for(let i=0;i<100;i++){try{ready=(await fs.readFile(handshake,'utf8')).trim()==='ready';}catch{}if(ready)break;await new Promise(r=>setTimeout(r,50));}
   assert.equal(ready,true);await new Promise(r=>setTimeout(r,200));await assert.rejects(fs.readFile(receipt));
   parent.kill();parent=null;
   assert.equal(await finished,0);
   const result=JSON.parse((await fs.readFile(receipt,'utf8')).replace(/^\uFEFF/,''));
   assert.equal(result.version,'0.2.1');assert.equal(result.exitCode,tampered?-1:scenario==='installer-failure'?17:0,JSON.stringify(result));
  }
 }finally{parent?.kill();await new Promise(r=>setTimeout(r,200));await fs.rm(dir,{recursive:true,force:true});}
});
