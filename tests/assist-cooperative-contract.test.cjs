"use strict";
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { stripTypeScriptTypes } = require("node:module");
const fixture = require("./fixtures/assist-control-v2-yield.json");
const source = name => fs.readFileSync(path.join(__dirname, "fixtures", name), "utf8");
const url = text => "data:text/javascript;base64," + Buffer.from(
  stripTypeScriptTypes(text, { mode: "transform" })).toString("base64");
const protocolUrl = url(source("assist-control-profile2.web.ts"));
const protocol = import(protocolUrl);
const cooperativeSource = source("assist-cooperative.web.ts");
const importPattern = /import \{[\s\S]*?\} from '\.\/protocol';/;
assert.ok(importPattern.test(cooperativeSource));
const cooperative = import(url(cooperativeSource.replace(importPattern, match =>
  match.replace("'./protocol'", JSON.stringify(protocolUrl)))));
const extensionSource = source("assist-native-ext-v2.web.ts");
const extension = import(url(extensionSource.replace("import { NATIVE_LIMITS } from './protocol';",
  "const NATIVE_LIMITS = Object.freeze({ messageChars: 32_768 });")));
const check = (fn, error) => error
  ? assert.throws(fn, e => e.code === error)
  : fn();

test("cooperative wire grammar and grant authorization match canonical fixtures", async () => {
  const api = await protocol;
  for (const m of fixture.messages.valid)
    assert.deepEqual(api.parseControlMessage(m, 2, new Set(fixture.messages.features)), m);
  for (const item of fixture.messages.invalid)
    check(() => api.parseControlMessage(item.message, item.version || 2,
      new Set(item.features || fixture.messages.features)), item.error);
  for (const item of fixture.messages.authorization)
    assert.equal(api.authorizes(item.kind, item.from, item.capability, item.version || 2), item.authorized);
});

for (const scenario of fixture.endpoint) test("canonical endpoint: " + scenario.name, async () => {
  const gate = (await cooperative).createLocalYield(fixture.session);
  for (const step of scenario.steps) {
    const run = () => step.helper ? gate.helper(step.helper, step.grant, step.at, step.ctx)
      : step.local ? gate.local(step.local, step.at, step.ctx)
      : step.end ? gate.end(step.end) : gate.tick(step.tick, step.ctx);
    if (step.error) check(run, step.error);
    else assert.deepEqual(run(), step.expect, step.name);
    assert.equal(gate.state, step.state);
  }
});

for (const scenario of fixture.helper) test("canonical helper: " + scenario.name, async () => {
  const follower = (await cooperative).createYieldFollower();
  for (const step of scenario.steps)
    assert.deepEqual(step.receive ? follower.receive(step.receive)
      : follower.canSend(step.canSend.release), step.expect, step.name);
});

test("extension v3 negotiates downward, retains resources and rejects excess versions", async () => {
  const api = await extension;
  assert.deepEqual(api.parseExtRequest(fixture.bridge.hello), fixture.bridge.hello);
  for (const item of fixture.bridge.results.valid)
    assert.deepEqual(api.parseExtResponse(item.response, "ext.hello", item.offered), item.response);
  for (const item of fixture.bridge.results.invalid)
    check(() => api.parseExtResponse(item.response, "ext.hello", item.offered), item.error);
  for (const m of fixture.bridge.events.valid) assert.deepEqual(api.parseExtEvent(m, 3), m);
  for (const item of fixture.bridge.events.invalid)
    check(() => api.parseExtEvent(item.message, item.version), item.error);
  for (const item of fixture.bridge.requests.valid)
    assert.deepEqual(api.parseExtRequest(item.message, item.version), item.message);
  for (const item of fixture.bridge.requests.invalid)
    check(() => api.parseExtRequest(item.message, item.version), item.error);
});
