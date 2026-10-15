# PCOC

**Early prototype.** Specification: Satoshi Hike v2.0 (`spec-v2.0.2`). Website: https://groundinglab.io

A PCOC (production-constrained observable commitment) is a self-issued identifier backed by a resource commitment, not a payment. You make it on your own device by holding memory through a hike paced by Bitcoin blocks. Anyone can check the file cheaply, without trusting you or us. The design aims to give bulk making no discount: copying the file is cheap, but a new public key means a new hike.

An identifier is not an identity: a PCOC says nothing about who made it.

## Why detection loses

Open systems usually defend against bots and fake accounts by detection: rules, filters, scores. A rule is solved once and the solution is reused by every account the attacker runs. The attacker pays once; the defence pays per user, forever. And volume is not aggression, so tightening the rule punishes heavy honest users along with the crowd.

Most of that trouble starts at issuance. Identifiers are either free and unlimited, which floods, or handed out by an office, which installs a gatekeeper. PCOC takes a third path: you issue your own, and the cost is in the making.

## What "self-issued" means

A PCOC is self-issued: you create it on your own device from your own key, with no issuer, registrar, or account to ask. What limits it is the resource commitment you make in producing it (memory held across a Bitcoin-paced hike), not a payment or anyone's permission. See [Where does the trouble start?](docs/briefs/where-does-the-trouble-start.md).

## Three separate steps

1. **Create (the hike).** Your computer fills a block of memory fixed by your public key and a Bitcoin block, then works through it in legs of two million steps, each ending in a leg result. Bitcoin blocks pace the stages: Satoshi Hike v2.0 records eighteen. The cost is your own device's work. Bitcoin only timestamps it: Bari records each tip's pooled waypoints in one Bitcoin transaction. Bitcoin's proof-of-work does not secure a PCOC or pay for it.
2. **Check (verify).** A verifier recomputes from the file and the public index and records measurements: `match`, `mismatch`, `present`, `skip`, `not defined`, `not read`, `not fetched by design`, `n/a`, `anchored`, `not anchored`, and `earlier version`. It never says pass or fail. The index stands in for Bitcoin; anyone can bypass it by checking Bitcoin directly.
3. **Admit (policy).** Accept, refuse, and revoke are each relying party's own decisions. groundinglab's Admission Gate is one example policy (its default leg-rate floor is 1/4, as locked in the specification). Policy can change without changing the file.

The Bitcoin tip is the shared public sequencer. Successive tips pace the stages and order them. That is the whole of its role: not a payment, not a confirmation count, and not an archive of the chain. The hike is the quantity of work.

## What this does not claim

- Bitcoin's proof-of-work does not secure or pay for a PCOC. Bitcoin is a public clock and timestamp, via Bari.
- It does not make fakes impossible. It raises the cost of each one above the near-free price of a new key. How that cost behaves at scale is still being measured ([STATUS](STATUS.md)).
- It is not a token, coin, currency, or payment.
- There is no issuer, account, or identity provider, and it says nothing about personhood.
- Verify does not judge; it measures. Admission belongs to each relying party.
- You do not need to trust groundinglab's index. Only hash-bound values are evidence.
- No privacy claim beyond the specification: the public key and its Bitcoin-timed record are public.
- Not affiliated with Bitcoin, its developers, or its creator. "Satoshi Hike" is named for the Bitcoin block clock it keeps pace with.
- Economic security, catch rates, and deployed cost are open research.

## How the repository fits together

The specification and the conformance vectors are the core deliverable. The point is interoperability. Everything else is built from them and pins a version of the layer below. Anyone can build a producer, a verifier, a service, or an app from the specification and the vectors alone, with no groundinglab code, service, or account.

| Layer | Folder | Role |
|---|---|---|
| Specification | [spec/](spec/) | The complete, normative definition of how a PCOC is made and checked |
| Conformance | [spec/conformance/](spec/conformance/v2.0/) | Shared vectors: what a Part A implementation must reproduce |
| References | [reference/](reference/) | First tested examples in Rust, Go, and Java. A guide, not a requirement. The specification wins if they disagree |
| Services | [services/](services/) | Bari, the index, and the Admission Gate, built on the Rust reference |
| Apps | [apps/](apps/) | Desktop demonstrator, and an Android demonstrator described in [apps/android](apps/android/) |
| Docs | [docs/](docs/README.md) | Plain-language briefs and rationale. Explanatory, not normative |

## Where next

- **Check one:** https://groundinglab.io/verify
- **Make one:** [desktop app](apps/desktop/)
- **Build one:** [spec](spec/) → [conformance](spec/conformance/v2.0/) → [reference](reference/)
- **Read why:** [briefs](docs/briefs/README.md) and [rationale](docs/README.md)
- **What is measured:** [STATUS](STATUS.md)
- **Contribute / report a vulnerability:** [CONTRIBUTING](CONTRIBUTING.md) · [SECURITY](SECURITY.md)

## Built on

Satoshi Nakamoto's timestamp server and attack economics (Bitcoin white paper §3, §4, §6, §11), Haber and Stornetta's linked timestamps, Dwork and Naor's pricing via computation and Adam Back's Hashcash, and memory-hard work (scrypt). The composition is ours.

## Licence

Code: Apache-2.0 ([LICENSE](LICENSE)). Specification and docs prose: CC BY 4.0 ([LICENSE-DOCS.md](LICENSE-DOCS.md)). Conformance vector bytes: Apache-2.0 ([LICENSE](LICENSE)).
