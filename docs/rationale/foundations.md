# Foundations

This is the picture. A PCOC object is a self-issued file. Making it cost something. It is bound to a key and to a public sequence the maker does not control. A stranger can check the relations from the file. A door may then decide whether those relations are enough. The family says what any construction still owes. A construction picks the numbers for a population. This page does not pick them.

## How it holds together

A key is free. The scarce step sits on the object. [How do you make a digital thing scarce?](../briefs/how-do-you-make-a-digital-thing-scarce.md)

The maker grows a terrain from the key and an opening flag, holds it, and takes the work. A later flag cuts the stretch. The terrain stays resident. The file is the log, not the terrain. [How you make one](how-you-make-one.md)

Binding is in that log. A checkpoint ties the run to the previous bind, to the stage mark, to a piece of the terrain, and to the result. A waypoint ties the stage to the flag that opened it. A prefix of a run is not a second object. The bytes of that bind are the construction's, not this page's. [the SPEC](https://github.com/groundinglab/pcoc/tree/spec-v2.0.1/spec/satoshi-hike-v2.0/Satoshi-Hike-v2.0.md)

The public sequence is the clock. Bitcoin is one system a construction may name. It does not issue the object, and it is not family law. [How do you prove when the work happened?](../briefs/how-do-you-prove-when-the-work-happened.md)

Verification reads the finished file. A cheap inspection may refuse. A confirming look re-does the runs it asks for, and still costs less than the making. A match establishes the relations. It is not admission. [Why is the check always cheaper than the making?](../briefs/why-can-saying-no-be-cheap-when-saying-yes-cannot.md) The reference that runs that check is [reference/rust/satoshi-hike-verify](https://github.com/groundinglab/pcoc/tree/ref-rust-v2.0.3/reference/rust/satoshi-hike-verify). Produce is [reference/rust/satoshi-hike-produce](https://github.com/groundinglab/pcoc/tree/ref-rust-v2.0.3/reference/rust/satoshi-hike-produce).

Admission is the door's policy. How many marks must sit on a chain it trusts, how old the object may be, whether one object is one seat: the object does not carry that line. A separate service, the [admission gate](https://github.com/groundinglab/pcoc/tree/gate-v0.3.2/services/admission-gate), applies a named policy to the measurements; the verifier does not. Its decision log is a companion ([WIRE.md](https://github.com/groundinglab/pcoc/blob/gate-v0.3.2/services/admission-gate/WIRE.md)), not a second construction.

Key ownership is a later act. The hike puts the public key in the seed so the terrain belongs to someone. It does not check a signature. A door that wants the presenter to hold the matching secret asks for that challenge itself. [Family contract](family-contract.md)

## A construction, a population, a floor

The family is a list of debts, not a parameter file. A concrete construction discharges them with its own magnitudes. Another may discharge the same ten with different ones. What it may not do is drop one. [Family contract](family-contract.md)

The construction this repository runs is one hike. Eighteen stages, two million steps a leg, two hundred and fifty-six mebibytes: those are its numbers, said once in [How you make one](how-you-make-one.md). They are not family law.

A construction is aimed at a population. The floor is what a seat costs to make in that population. Two holes remain. Someone makes a file that checks as valid for less than the floor. Or the floor holds and the room cannot afford to look. A file that did the work is valid. [Threat model](threat-model.md)

The costs are two bills. The maker pays once per object. The checker pays once per file looked at. A skip counted twice is not a measurement. No price is printed here. [Cost and incidence](cost-and-incidence.md)

## Where a name goes

The briefs are the questions. They teach. They do not amend a specification.

A cap and a list are the same office. [Issued, listed, or made?](../briefs/issued-listed-or-made.md)

The cost does not amortise. [What kind of cost refuses to divide?](../briefs/what-kind-of-cost-refuses-to-divide.md)

Occupancy is the unit. The time half is not in the bytes. [Why memory, and why a clock?](../briefs/why-memory-and-why-a-clock.md)

The evidence is in the file, not in a lookup. [Can you check a stranger without asking anyone?](../briefs/can-you-check-a-stranger-without-asking.md)

The object is already a composition. The uses are a guess. [What follows from two things?](../briefs/what-follows-from-two-things.md)

The finished file is a spent commitment. Keeping it is optional. [Why carry the cost forward?](../briefs/why-carry-the-cost-forward.md)

## Where the parts sit

[BARI](../../services/bari/) is the notary. It observes headers and can plant a mark. It does not correct the hike, and a missed plant is an unattested path, not an invalid object. Bari writes the index files for that mark and does not serve them. The plant is a fee. The work is the occupancy. They are not the same bill.

The specification is the hike as text. A pin change is not a new family debt. [spec](../../spec/)

Tests and conformance are how a second implementation shows it agrees. They are not the argument. [conformance](../../spec/conformance/) Nothing under it is this law.
