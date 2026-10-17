<!--
Nostr long-form (NIP-23, kind 30023) version of keeping-open-systems-open.md.
Everything above the line "ARTICLE BODY STARTS BELOW" is posting metadata and notes.
Paste only the body (below that line) into the editor's Markdown field.
-->

## Posting metadata (NIP-23 tags)

```text
title:    Free Keys, Fake npubs: Keeping Nostr Open Without Giving Control Away
summary:  On Nostr anyone can make a key for free, and so can a bot, thousands of times over. A PCOC is a self-issued identifier whose cost is the maker's own work, with Bitcoin as the clock, so fakes get expensive without adding a gatekeeper.
t:        nostr, spam, identity, bitcoin, pcoc
image:    none (optional: add a banner later; the article does not need one)
d:        free-keys-fake-npubs
```

## How to post it

- **Primal web:** go to Reads and choose New Article. Fill in the title, summary and (optional) image, then paste the Markdown body. Primal also has a fuller article editor in Primal Studio.
- **YakiHonne or Habla (web):** both have long-form editors with fields for title, summary, tags and a banner image, plus a Markdown body.
- **Amethyst:** its code supports publishing NIP-23 long-form (title, summary, image, hashtags, d tag). The exact menu path was not confirmed, so check the app's compose options.
- Some editors set the `d` slug themselves. If yours lets you set it, use `free-keys-fake-npubs`, and reuse it whenever you edit so the update replaces the old version.
- Don't put a `#` title line in the body. The title tag is shown as the heading.

## Announcement note (kind 1)

```text
Free keys are Nostr's superpower, and its spam problem. New article: how a cost-backed identifier makes bot armies pay in real work, with no gatekeeper and no payment.
nostr:naddr1... (paste the article's link here after publishing)
```

---

ARTICLE BODY STARTS BELOW

Anyone can make a Nostr key. No signup, no email, no phone number, nobody to ask. That's the whole point, and it's why a lot of us are here.

It's also the dark side. If you can make a key for free, so can a bot. Bots and AI agents can now make thousands of keys in the time it takes you to make one, and every new npub costs them next to nothing. Openness becomes an attack surface. Reply spam, fake accounts and impersonators all start from the same place: a new identity is free and unlimited.

An open network still has to work when lots of its keys are fake. The question is how to do that without closing the door.

## Why filters keep losing

Most defences today are detection: rules, filters, scores, block lists. The trouble is that a rule only has to be beaten once. After that, every bot the attacker runs uses the same trick. The attacker pays once, and the defence pays for every user, forever.

Tightening the rules doesn't fix it either. Posting a lot isn't the same as being an attacker, so stricter filters end up hitting heavy honest users along with the bots.

## The usual fixes hand control to someone else

Back in 2016, Christopher Allen's [The Path to Self-Sovereign Identity](https://www.lifewithalacrity.com/article/the-path-to-self-soverereign-identity/) set out what must not be given away: you stay the ultimate authority over your own identity, you consent to how it's used, and you disclose only what you choose. Nostr gets a lot of that right. Your key is yours. But those principles don't put a cost on making extra identifiers, and that's exactly the gap bots use.

When open systems get flooded with cheap identifiers, the common responses move the decision of who gets in away from you:

- An issuer decides which identifiers are valid.
- A biometric check makes you hand a physical measurement to an outside service.
- A social-graph score rates accounts by the connections the scoring system can see.
- A detection system keeps changing rules you neither control nor fully see.

Each one cuts down bots. Each one also takes away some of your control. The cost of an extra identifier stops being something the maker pays and becomes a condition someone else sets.

So here's the real question: can we raise the cost of each extra identifier without moving that decision away from you?

## A cost you pay yourself

That's what a PCOC is for. A PCOC is a self-issued identifier backed by a resource commitment, not a payment. You make it on your own device, from your own key, without asking anyone. There's no issuer, no registrar and no account.

The cost is your own device's work: it holds a block of memory and works through it in legs, and the stages are paced by Bitcoin blocks. Bitcoin is only the clock here, a shared public tick that nobody controls. There's no wallet, no zap and no sats involved, and nobody gets paid.

Because the cost is memory held across Bitcoin blocks, you can't rush it by throwing more hashrate at it. Copying a finished file is cheap, but a new key means doing the work again. So one identifier is easy for a person, while a bot farm that wants ten thousand has to do the work ten thousand times.

The result is a small file anyone can check cheaply, without trusting you or groundinglab. Checking is far cheaper than making. Any index the checker reads can be rebuilt from Bitcoin itself. It discloses no personal data, and an identifier is not an identity: a PCOC says nothing about who made it.

## Measurement here, policy there

Checking a PCOC measures whether it was made the way the specification says, at the pace it says. It doesn't say pass or fail.

What counts as enough is up to each relay, client or service. groundinglab runs one example, an Admission Gate, but its policy is just a recommendation. Anyone can set their own, and policy can change without changing the file. You keep your identifier, and each relying system keeps its own rules.

## Why Nostr first, and why this isn't a Nostr thing

We're starting on Nostr because it's open. Anyone can build on it, and nobody has to give permission for something like this to plug in.

But this isn't a Nostr project. The problem is general: any open system where identifiers are free has it. Nostr just feels it most, because it's one of the most open systems around. Free keys are the feature, and fake npubs are the price. Without something like cost-backed identifiers, Nostr will keep struggling with spam and fakes, or drift toward the gatekeepers it was built to avoid.

The specification is open and built for interoperability. Anyone can build a maker, a checker or a service from it, with no groundinglab code, service or account, and it can be bound to other credential systems as a rate limit on making identifiers. Nostr is the first test. If it holds up here, other open systems with the same problem can use it.

## Try it

There's a working demonstrator today: a fork of Primal's Android client that lets you make a PCOC on your phone, check one, and sign in with one. It's a groundinglab demo, not an official Primal release, and it isn't endorsed by Primal. The APK is debug-signed and for side-loading only. It's on the [demonstrations page](https://groundinglab.io/demonstrations/).

- Check a PCOC: https://groundinglab.io/verify
- Specification, references and code: https://github.com/groundinglab/pcoc

This is an early prototype, and open questions remain: how the cost holds up against large farms, what policies relays and clients will choose, and how this fits with other uniqueness mechanisms. Cost at scale is still being measured. If you think it falls short somewhere, tell us. That's what this stage is for.
