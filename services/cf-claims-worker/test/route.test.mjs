import assert from "node:assert/strict";
import test from "node:test";
import worker, { nip05Answer, objectFor } from "../src/index.js";

test("paths map to bucket keys and unknown paths do not redirect", () => {
  assert.equal(objectFor("/.well-known/nostr.json").key, ".well-known/nostr.json");
  assert.equal(objectFor("/.well-known/nostr.json").cache, "no-store");
  assert.equal(objectFor("/claims/index.json").cache, "public, max-age=30");
  const id = "ab".repeat(32);
  assert.equal(objectFor(`/claims/${id}.json`).key, `claims/${id}.json`);
  assert.match(objectFor(`/claims/${id}.json`).cache, /immutable/);
  assert.equal(objectFor("/badge/npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq").key.endsWith(".html"), true);
  assert.equal(objectFor("/claims/../secret"), null);
  assert.equal(objectFor("/badge/npub1example/"), null);
  assert.equal(objectFor("/log/2026-10-14.jsonl"), null);
});

test("GET sets CORS and a miss is 404 with no location header", async () => {
  const stored = new Map([
    [".well-known/nostr.json", { body: "{\"names\":{}}", async text() { return "{\"names\":{}}"; } }],
  ]);
  const env = { CLAIMS: { async get(key) { return stored.get(key) || null; } } };
  const ok = await worker.fetch(new Request("https://groundinglab.io/.well-known/nostr.json?name=_probe"), env);
  assert.equal(ok.status, 200);
  assert.equal(ok.headers.get("access-control-allow-origin"), "*");
  assert.equal(ok.headers.get("content-type"), "application/json");
  assert.equal(ok.headers.get("location"), null);
  const missing = await worker.fetch(new Request("https://groundinglab.io/claims/index.json"), env);
  assert.equal(missing.status, 404);
  assert.equal(missing.headers.get("cache-control"), "no-store");
  assert.equal(missing.headers.get("location"), null);
  const posted = await worker.fetch(new Request("https://groundinglab.io/claims/index.json", { method: "POST" }), env);
  assert.equal(posted.status, 405);
  assert.equal(posted.headers.get("location"), null);
});

test("nostr.json answers only the requested name", async () => {
  const a = "aa".repeat(32);
  const b = "bb".repeat(32);
  const stored = { names: { alice: a, bob: b }, relays: { [a]: ["wss://relay.example"] } };
  assert.deepEqual(nip05Answer(stored, "alice"), { names: { alice: a }, relays: { [a]: ["wss://relay.example"] } });
  assert.deepEqual(nip05Answer(stored, "bob"), { names: { bob: b } });
  assert.deepEqual(nip05Answer(stored, "carol"), { names: {} });
  assert.deepEqual(nip05Answer(stored, null), { names: {} });
  assert.deepEqual(nip05Answer(stored, "constructor"), { names: {} });
  assert.deepEqual(nip05Answer(stored, "Alice"), { names: {} });

  const env = { CLAIMS: { async get() { return { body: null, async text() { return JSON.stringify(stored); } }; } } };
  for (const [query, expected] of [
    ["?name=bob", { names: { bob: b } }],
    ["?name=nobody", { names: {} }],
    ["", { names: {} }],
  ]) {
    const res = await worker.fetch(new Request(`https://groundinglab.io/.well-known/nostr.json${query}`), env);
    assert.equal(res.status, 200);
    assert.equal(res.headers.get("content-type"), "application/json");
    assert.equal(res.headers.get("access-control-allow-origin"), "*");
    assert.equal(res.headers.get("cache-control"), "no-store");
    assert.deepEqual(await res.json(), expected);
  }
  const head = await worker.fetch(new Request("https://groundinglab.io/.well-known/nostr.json?name=bob", { method: "HEAD" }), env);
  assert.equal(head.status, 200);
  assert.equal(await head.text(), "");
});
