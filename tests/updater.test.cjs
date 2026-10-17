'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs/promises');
const path=require('node:path');
const os=require('node:os');
const {EventEmitter}=require('node:events');
async function harness(fetchRelease,downloadInstaller,verifyDownloadedFile=async()=>true){
 const directory=await fs.mkdtemp(path.join(os.tmpdir(),'palisade-controller-'));
 const app=Object.assign(new EventEmitter(),{isPackaged:true,getPath:()=>directory,getVersion:()=> '0.2.1',quit:()=>{throw new Error('Unexpected installation');}});
 const prompts=[],statuses=[],progress=[],timers=[];
 const electron={app,dialog:{showMessageBox:async(...args)=>{prompts.push(args.at(-1));return {response:1};}},Notification:class{static isSupported(){return false;}}};
 const exports={};
 const context={module:{exports},exports,__dirname:path.join(__dirname,'../src/updates'),process:{platform:'win32'},fetch:()=>{},setTimeout:(f,ms)=>{timers.push(ms);return {unref(){}};},setInterval:(f,ms)=>{timers.push(ms);return {unref(){}};},clearTimeout(){},clearInterval(){},require:(name)=> name==='electron'?electron:name==='./release.cjs'?{fetchRelease,verifyDownloadedFile}:name==='./downloader.cjs'?{downloadInstaller}:require(name)};
 vm.runInNewContext(await fs.readFile(path.join(__dirname,'../src/updates/updater.cjs'),'utf8'),context);
 const controller=context.module.exports.createUpdateController(()=>({isDestroyed:()=>false,setProgressBar:v=>progress.push(v)}),s=>statuses.push(s));
 return {controller,prompts,statuses,progress,timers,app,cleanup:()=>fs.rm(directory,{recursive:true,force:true})};
}
test('controller offers postponement, progress and ready status; a postponed installer is reused',async()=>{
 let downloads=0;
 const h=await harness(async()=>({version:'0.2.2',releaseNotes:'Release details'}),async(_m,dir,_f,onProgress)=>{downloads++;onProgress(5,10);onProgress(10,10);return path.join(dir,'installer.exe');});
 try{
  await h.controller.check(true);
  assert.equal(downloads,1);assert.match(h.prompts[0].detail,/Release details/);
  assert.equal(h.prompts[0].buttons[1],'Later');assert.ok(h.progress.includes(0.5));assert.equal(h.progress.at(-1),-1);
  await h.controller.check(true);assert.equal(downloads,1);assert.equal(h.statuses.at(-1),'Update 0.2.2 ready');
 }finally{await h.cleanup();}
});
test('controller reports network failure and recovers on a subsequent check',async()=>{
 let calls=0;
 const h=await harness(async()=>{if(!calls++)throw new Error('offline');return null;},()=>{throw new Error('unexpected download');});
 try{await h.controller.check(true);assert.match(h.statuses.at(-1),/failed/);assert.equal(h.prompts[0].type,'warning');await h.controller.check(true);assert.equal(h.statuses.at(-1),'Updates: up to date');}finally{await h.cleanup();}
});
test('checks are serialized and shutdown cancels future checks; schedules startup and six-hour checks',async()=>{
 let finish;let calls=0;let started;
 const fetching=new Promise(resolve=>{started=resolve;});
 const h=await harness(()=>{calls++;started();return new Promise(r=>{finish=r;});},()=>{});
 try{
  h.controller.start();assert.deepEqual(h.timers,[15000,21600000]);
  const pending=h.controller.check();await fetching;await h.controller.check(true);assert.equal(calls,1);
  finish(null);await pending;h.app.emit('before-quit');await h.controller.check(true);assert.equal(calls,1);
 }finally{await h.cleanup();}
});
