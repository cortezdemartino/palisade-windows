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
  assert.equal(provenance.commit,"5b82bc90e4997f35dd18273427c258fe54eefb95");
  const verified = new Set();
  for(const file of provenance.files) {
    const bytes = Buffer.from(fs.readFileSync(path.join(__dirname,"..",file.local),"utf8").replace(/\r\n/g,"\n"));
    assert.equal(createHash("sha1").update("blob "+bytes.length+"\\0".replace("\\0","\0")).update(bytes).digest("hex"),file.gitBlob,file.source);
    const filename = path.basename(file.local);
    if(Object.hasOwn(provenance.fixtureSha256,filename)) {
      assert.equal(createHash("sha256").update(bytes).digest("hex"),provenance.fixtureSha256[filename],file.local);
      verified.add(filename);
    }
  }
  assert.deepEqual([...verified].sort(), Object.keys(provenance.fixtureSha256).sort());
});

test("canonical v2 release authority and ownership match every shared scenario", async () => {
  const api = await canonical;
  const fixture = require("./fixtures/assist-control-v2-release.json");
  assert.deepEqual(api.RELEASE_SCOPE, fixture.releaseScope);
  assert.equal(api.authorizes("release-all", "guest", "control_mouse", 1), false);
  assert.equal(api.authorizes("release-all", "guest", "control_keyboard", 1), true);
  for (const scenario of fixture.scenarios) {
    const owned = api.createOwnedInput(fixture.session);
    for (const step of scenario.steps) {
      if (step.error) {
        assert.throws(() => owned.apply(step.message, step.grant), e => e.code === step.error, step.name);
      } else {
        assert.deepEqual(owned.apply(step.message, step.grant), step.expect, step.name);
      }
    }
    if (scenario.teardown) assert.deepEqual(owned.releaseEverything(), scenario.teardown);
  }
});

test("interop manifest agrees with every pinned fixture and preserves the acceptance inventory", () => {
  const manifest = require("./fixtures/assist-interop-manifest.json");
  const files = {
    "control-v1": "assist-control-v1.json", "control-v2": "assist-control-v2.json",
    "control-v2-schema": "assist-control-v2.schema.json", "control-v2-profile2": "assist-control-v2-profile2.json",
    "control-v2-release": "assist-control-v2-release.json", "native-v1": "assist-native-v1.json",
    "native-ext-v1": "assist-native-ext-v1.json", "native-ext-v2": "assist-native-ext-v2.json",
  };
  assert.deepEqual(Object.keys(manifest.fixtures).sort(), Object.keys(files).sort());
  for (const [family, local] of Object.entries(files)) {
    const bytes = Buffer.from(fs.readFileSync(path.join(root, local), "utf8").replace(/\r\n/g, "\n"));
    assert.equal(createHash("sha256").update(bytes).digest("hex"), manifest.fixtures[family].sha256, family);
    assert.equal(createHash("sha1").update("blob " + bytes.length + "\0").update(bytes).digest("hex"), manifest.fixtures[family].gitBlob, family);
  }
  assert.equal(manifest.sources["apps/web/src/assist/control/protocol.ts"].gitBlob,
    provenance.files.find(f => f.source === "apps/web/src/assist/control/protocol.ts").gitBlob);
  assert.equal(manifest.acceptance.length, 23);
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
