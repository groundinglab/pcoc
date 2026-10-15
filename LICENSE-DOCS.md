# Documentation licence

Prose is licensed under the Creative Commons Attribution 4.0 International Licence (CC BY 4.0).

Scope: Markdown under `spec/` (including `spec/conformance/**/*.md`), `docs/`, the repository root, and per-folder READMEs, including `STATUS.md`.

Summary: https://creativecommons.org/licenses/by/4.0/

Legal code: https://creativecommons.org/licenses/by/4.0/legalcode

Code stays Apache-2.0. See [LICENSE](LICENSE).

Conformance vector bytes are Apache-2.0. That is the rule in force. The bytes are `spec/conformance/v2.0/*.json`, `spec/conformance/v2.0/earlier-kind.cbor`, and `spec/conformance/blockhash-display.json`. They are a test oracle: the Rust reference regenerates them, `reference/SPEC_PIN` hashes them, and the references compare them. Prose beside those bytes stays CC BY 4.0. The byte-versus-prose split inside `spec/conformance/` is recorded in this file.
