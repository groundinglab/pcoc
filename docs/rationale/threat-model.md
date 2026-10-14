# Threat model

Two holes. Someone makes a file that checks as valid for less than the floor. Or the floor holds, and the room cannot afford to look. A file that did the work is valid. Valid is the construction working, not a third hole.

A key is free. Anyone can make one, and making a million costs no more per key than making one. A seat is not free. The seat is an object: work done on a machine, bound to that key, marked against a public sequence. Whoever holds one is in the population, and nobody hands them out. [Family contract](family-contract.md)

So the arrangement rests on a single number: what a seat costs to make. Call it the floor.

Anyone may try to make one, and what comes out valid is valid. If the work was done, the floor held. There are only two ways it fails. Someone makes a file that checks as valid for less than the floor. Or the floor holds and the room cannot afford to look.

One hole is in the floor. The other is in the ability to see the floor. Both end the same way, and they need separate answers. Raising the floor does nothing for the second. Cheapening the checks does nothing for the first.

## Cheaper seats

The routes are not exotic. The work can be skipped and the shape kept. One making can be presented as many files. The machine can be rented rather than owned. The sequence can be collected as headers arrive and the work piled into the last few of them, so the marks look paced when they were not.

A bug takes the same path. So does a second implementation that read the specification differently and produced something honest and wrong. Nothing here distinguishes a cheat from a mistake. The path is the fact. Who made it is not.

What makes a route worth closing depends on who is making it. A cost that stops one party is nothing to another, and three things decide which.

The weighing is the one Jason P. Lowery sets out in *Softwar* (MIT thesis, 2023): a party attacks when the benefit of attack exceeds its cost.[^softwar] An attacker weighs the benefit of attacking against the cost (white paper §6). For PCOC the benefit is not something the holder gets from the identifier itself. It is admission: a seat in a population that a door lets in only at a price. The attack is getting many seats into that population cheaply. PCOC makes each seat cost real, measurable production work: memory-bound work timed between Bitcoin blocks, so many seats cost many times what one costs, while checking a seat stays cheap. It is not Bitcoin's proof-of-work. Something that used to be free to mint now has a cost to make, and because no one issues it, no issuer's policy or capacity limits how many can exist. Whether a given floor outweighs a given party's benefit is each door's policy, not something a check reports.

What they want. Wanting a fabricated file accepted is not the same as wanting only to keep the checkers busy. A floor high enough to discourage the first can be wide open to the second.

What they can spend. Own money behaves differently from rented capacity, and differently again from machines nobody is paying for. Which meter each party is actually paying is [Cost and incidence](cost-and-incidence.md).

What a seat is worth to them. If holding one gains them little, a cheaper route can be entirely real and still not worth an afternoon.

Leave any of the three unnamed and the floor is a number aimed at nobody. No floor can be shown to be too low in the abstract, because too low always depends on who was asking. Whether this construction's floor holds against a well-funded party is not established on this page.

## Looking gets too expensive

The second hole opens without anyone attacking anything.

A cheap look can refuse a file that is not this object. Wrong shape, a header that was never that header, a mark that does not follow from the stage just closed, a checkpoint that does not hash its limbs. Those refusals do not replay the terrain. They are not a yes. A confirming look re-does the runs it asks for, and that look is still cheaper than the making. [Why is the check always cheaper than the making?](../briefs/why-can-saying-no-be-cheap-when-saying-yes-cannot.md)

The hole is not that confirming costs as much as producing. The hole is a room that will only look if every arrival costs a full replay. That room stops looking, and a floor nobody can afford to check protects nothing. A sample that mismatches is enough for a gate to refuse the file. A sample that matches is the sampled relation, not a judgment that the object should be accepted, and not a claim that the undrawn spans were taken.

It is tempting to read a cheap rejection as evidence that someone tried something expensive and got caught. It is not. The rejection was cheap because it skipped the work, and a faker skips exactly the same work while making the file. Two savings on the same omission are not a ratio between an attacker and a defender. They are the same saving, counted twice.

A file can match on the cheap checks having never done the legs, and show a mismatch only when the work itself is replayed. Cheap and matching, at the same time.

## What can be seen

From the bytes, a stranger can establish that the file is this construction, that its structure holds, and that the sampled spans match, as far as the chosen level checks. From the public sequence, the same stranger can read whether the headers are the headers they claim. That comparison is not a question to the maker.

Nothing outside that list is prevented. Specialised hardware, rented capacity, one run reused as many files, work deferred inside a stage: where a checker cannot see them, they stay open. No number is put against any of them here.

The sequence is borrowed, and its behaviour stays with it. Depth, forks, and rewrites are properties of the carrier, not powers of the object. When the required marks are missing, the record shows them as skips. The object is not treated as confirmed from the marked legs. Bytes-only structure checks may still refuse junk. They do not stand in for a sequenced pass. The object is not repaired, and no settlement guarantee is claimed on its behalf.

A rejected file costs its maker exactly that file. No consequence outlives the rejection, so detection does not accumulate. Today's catch makes tomorrow no harder. Whether to accept anything remains a matter of policy, and the check was never the policy. [Family contract](family-contract.md)

This is one lock, on one floor, aimed at ordinary machines in wide use. A different population needs a different floor, and a different lock.

[^softwar]: Jason P. Lowery, *Softwar: A Novel Theory on Power Projection and the National Strategic Significance of Bitcoin*, S.M. thesis, MIT System Design and Management, February 2023. http://hdl.handle.net/1721.1/153030
