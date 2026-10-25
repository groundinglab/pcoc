# Continue with PCOC button

Minimum concrete steps when the human asks to add a **Continue with PCOC** control. Do not invent routes, fields, or policy thresholds. Read the documents listed at the end before editing.

## What the button is

- Label: `Continue with PCOC`.
- Place it beside the site’s existing sign-in controls. It is not a third-party account login and does not mint an OAuth session.
- The user already holds a PCOC object on their device. The button starts the locked sign-in submission to this site’s admission gate.

## Client → gate

1. Build the locked SPEC §15.1 six-field JSON body and `POST` it to `POST /v1/sign-in` with `Content-Type: application/json`. The six keys appear once: `origin`, `nonce`, `exp`, `object`, `object_b64`, `signature`. See `services/admission-gate/WIRE.md` (Locked body) and SPEC §15.1.
2. Handle the three client bodies. There is no `message` field:
   - `{"delivery":"ok"}` → show **Sign-in submitted**.
   - `{"error":{"code":"bad_signature"}}`
   - `{"error":{"code":"malformed"}}`
3. Treat `{"delivery":"ok"}`, and a provisional grant that has not settled, as signed in immediately for the site. Poll `GET /v1/sign-in/result/{nonce}` (64 lowercase hex) until the outcome leaves `pending` (`accepted`, `rejected`, or another terminal token the wire documents).
4. A consumed provisional still pending looks like `{"outcome":"pending","submitted":true}`. Do **not** treat `{"outcome":"rejected"}` without `submitted` as a provisional grant (banned resubmit and similar).
5. Keep policy out of the button code. Thresholds and levers live in a human-editable policy file (`policy/README.md` plus the file the human names). Do not hard-code floors, caps, or refuse defaults.

## Documents to read

- `spec/04-Admission_Process.md`
- `services/admission-gate/WIRE.md`
- `policy/README.md` and the policy file the human names
- Site guides: https://groundinglab.io/technical/sign-in-wire/ and https://groundinglab.io/technical/integrate-an-admission-gate/

Wire and provisional behaviour above match `services/admission-gate/WIRE.md` as of this commit. If that wire changes, update this file in the same change.
