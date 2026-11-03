/**
 * GET-only reader for the groundinglab claims bucket.
 * Paths: /claims/*, /badge/*, /.well-known/nostr.json.
 * No redirects. CORS is *.
 */

const CORS = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET, HEAD, OPTIONS",
};

export function objectFor(pathname) {
  if (pathname.includes("..") || pathname.includes("\\") || pathname.includes("%")) {
    return null;
  }
  if (pathname === "/.well-known/nostr.json") {
    return { key: ".well-known/nostr.json", type: "application/json", cache: "no-store", nip05: true };
  }
  if (pathname === "/claims/index.json") {
    return { key: "claims/index.json", type: "application/json", cache: "public, max-age=30" };
  }
  const claim = /^\/claims\/([0-9a-f]{64})\.json$/.exec(pathname);
  if (claim) {
    return {
      key: `claims/${claim[1]}.json`,
      type: "application/json",
      cache: "public, max-age=31536000, immutable",
    };
  }
  const badge = /^\/badge\/(npub1[02-9ac-hj-np-z]+)$/.exec(pathname);
  if (badge) {
    return {
      key: `badge/${badge[1]}.html`,
      type: "text/html; charset=utf-8",
      cache: "public, max-age=31536000, immutable",
    };
  }
  return null;
}

function headers(extra) {
  return { ...CORS, ...extra };
}

// NIP-05: answer only the name asked for, so one lookup does not list every name.
export function nip05Answer(stored, name) {
  const names = (stored && typeof stored.names === "object" && stored.names) || {};
  const out = { names: {} };
  if (typeof name !== "string" || !Object.prototype.hasOwnProperty.call(names, name)) return out;
  const hex = names[name];
  out.names[name] = hex;
  const relays = stored.relays && typeof stored.relays === "object" ? stored.relays : null;
  if (relays && Object.prototype.hasOwnProperty.call(relays, hex)) {
    out.relays = { [hex]: relays[hex] };
  }
  return out;
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const object = objectFor(url.pathname);
    if (request.method === "OPTIONS") {
      if (!object) return new Response(null, { status: 404, headers: headers({ "cache-control": "no-store" }) });
      return new Response(null, { status: 204, headers: headers({ "cache-control": "no-store" }) });
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      return new Response(null, {
        status: 405,
        headers: headers({ allow: "GET, HEAD, OPTIONS", "cache-control": "no-store" }),
      });
    }
    if (!object) {
      return new Response(null, { status: 404, headers: headers({ "cache-control": "no-store" }) });
    }
    const found = await env.CLAIMS.get(object.key);
    if (!found) {
      return new Response(null, { status: 404, headers: headers({ "cache-control": "no-store" }) });
    }
    let body = request.method === "HEAD" ? null : found.body;
    if (object.nip05) {
      let stored;
      try {
        stored = JSON.parse(await found.text());
      } catch {
        return new Response(null, { status: 502, headers: headers({ "cache-control": "no-store" }) });
      }
      const text = JSON.stringify(nip05Answer(stored, url.searchParams.get("name")));
      body = request.method === "HEAD" ? null : text;
    }
    return new Response(body, {
      status: 200,
      headers: headers({
        "content-type": object.type,
        "cache-control": object.cache,
      }),
    });
  },
};
