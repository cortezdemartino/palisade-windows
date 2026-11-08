"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { createHash } = require("node:crypto");
const { stripTypeScriptTypes } = require("node:module");
const root = path.join(__dirname, "fixtures");
const provenance = require("./fixtures/assist-profile2.provenance.json");
const profile = require("./fixtures/assist-control-v2-profile2.json");
const ext = require("./fixtures/assist-native-ext-v2.json");
const nativeAge = require("../src/assist/input-age.cjs");
function compile(src) {
  return import("data:text/javascript;base64," +
    Buffer.from(stripTypeScriptTypes(src, { mode: "transform" })).toString("base64"));
}
const canonical = compile(fs.readFileSync(path.join(root,"assist-control-profile2.web.ts"),"utf8"));
const extensionSource = fs.readFileSync(path.join(root,"assist-native-ext-v2.web.ts"),"utf8");
const fixedImport = "import { NATIVE_LIMITS } from './protocol';";
if (!extensionSource.startsWith(fixedImport)) throw Error("Canonical extension import changed");
const extension = compile(extensionSource.replace(fixedImport,
  "const NATIVE_LIMITS = Object.freeze({ messageChars: 32_768 });"));
function check(fn, expected) {
  try { fn(); assert.equal(expected, "accepted"); }
  catch(e) { if (e.code === expected) return; throw e; }
}
test("pin exact web commit, Git blobs and fixture SHA256", () => {
  assert.equal(provenance.commit,"ae3542fc0adb9d7f19b3e9a8521fb039555ee7f6");
  for(const file of provenance.files) {
    const bytes = Buffer.from(fs.readFileSync(path.join(root,file.local),"utf8").replace(/\r\n/g,"\n"));
    assert.equal(createHash("sha1").update("blob "+bytes.length+"\\0".replace("\\0","\0")).update(bytes).digest("hex"),file.gitBlob,file.source);
    if(Object.hasOwn(provenance.fixtureSha256,file.local))
      assert.equal(createHash("sha256").update(bytes).digest("hex"),provenance.fixtureSha256[file.local],file.local);
  }
});
test("web profile 2 accepts exact valid and refuses invalid kinds",async()=>{
  const api = await canonical;
  for(const item of profile.messages.valid) {
    check(()=>api.parseControlMessage(item.message,2), "accepted");
    check(()=>api.parseControlMessage(item.message,2,new Set([item.feature])), "accepted");
  }
  const all = new Set(["input2","clipboardImage","clipboardSync","fileResume"]);
  for(const item of profile.messages.invalid) {
    const m = item.textCodeUnits ?
      {...item.message,text:String.fromCharCode(...item.textCodeUnits)}:item.message;
    check(()=>api.parseControlMessage(m,item.version,new Set(item.features||all)),item.error);
  }
});
test("native monotonic input-age policy matches all web fixture sequences",async()=>{
  const web=await canonical;
  for(const scenario of profile.inputAge.sequences){
    const ours=nativeAge.createInputAgeGuard(profile.inputAge.maxAgeMs);
    const theirs=web.createInputAgeGuard(profile.inputAge.maxAgeMs);
    for(const ev of scenario.events) {
      check(()=>ours(ev.t,ev.arrival,ev.release),ev.expect);
      check(()=>theirs(ev.t,ev.arrival,ev.release),ev.expect);
    }
  }
  const peerA=nativeAge.createInputAgeGuard();
  check(()=>peerA(10,1000,false),"accepted");
  check(()=>peerA(9,1010,true),"replay");
  const peerB=nativeAge.createInputAgeGuard();
  check(()=>peerB(0,100000,false),"accepted");
  check(()=>peerB(-1,100001,false),"invalid-message");
});
test("web extension v2 preserves separately negotiated v1 hello and v2 payload rules",async()=>{
  const api=await extension;
  for(const m of ext.requests.valid)
    check(()=>api.parseExtRequest(m,m.op==="ext.hello"?1:2),"accepted");
  for(const c of ext.requests.invalid)
    check(()=>api.parseExtRequest(c.message,c.version),c.error);
  for(const r of ext.responses.valid)
    check(()=>api.parseExtResponse(r.message,r.op,2),"accepted");
  for(const r of ext.responses.invalid)
    check(()=>api.parseExtResponse(r.message,r.op,2),r.error);
  for(const m of ext.events.valid)
    check(()=>api.parseExtEvent(m,2),"accepted");
  for(const r of ext.events.invalid)
    check(()=>api.parseExtEvent(r.message,2),r.error);
});
test("no page-provided authority, automatic enablement or unsafe feature advertisement",()=>{
  const main=fs.readFileSync(path.join(__dirname,"../src/main.cjs"),"utf8");
  const live=fs.readFileSync(path.join(__dirname,"../src/assist/live-channel.cjs"),"utf8");
  assert.doesNotMatch(main,/installNativeBridge\(/);
  assert.match(live,/cleanupScope === 'channel-only'/);
  assert.deepEqual(ext.safety.ops.sort(), ["resource.cancel","resource.sync(state: off)"].sort());
});