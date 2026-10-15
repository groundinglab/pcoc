import assert from "node:assert/strict";
import test from "node:test";
import worker, { objectFor } from "../src/index.js";

test("paths map to bucket keys and unknown paths do not redirect", () => {
  assert.equal(objectFor("/.well-known/nostr.json").key, ".well-known/nostr.json");
  assert.equal(objectFor("/.well-known/nostr.json").cache, "public, max-age=30");
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
    [".well-known/nostr.json", { body: "{\"names\":{}}" }],
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
