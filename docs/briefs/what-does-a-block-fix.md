# What does a block fix?

Data has a where. A block also has a when.

Information is physical. Rolf Landauer made the point in 1961, in "Irreversibility and Heat Generation in the Computing Process": a bit is held in some substrate, and erasing it has a minimum energy cost. A record is a state of matter. Landauer's bound is tiny, so it is the framing here, not the source of any cost below.

Most records stop there. Nothing in the bytes says when they were written.

A Bitcoin block is a record of a different kind. It is fixed in space: copied to every node, and anyone can check that each copy matches. It is also fixed in time. Each block names the one before it, and arrives on a pace no single party controls. Rewinding it means redoing its work and the work of every block after it. Nobody can know its hash in advance. So a block is not only data. It is data at a moment, on a public clock nobody owns. [How do you prove when the work happened?](how-do-you-prove-when-the-work-happened.md)

None of this is new. Satoshi Nakamoto's Bitcoin paper (2008) calls the network a distributed timestamp server, built on Haber and Stornetta's linked timestamps (1991) and proof-of-work similar to Adam Back's Hashcash. Bonneau, Clark and Goldfeder (2015) studied block hashes as a public randomness beacon, and showed that biasing one is possible but has a cost that can be bounded. This page uses the plain description: a public tick, fixed in space and time, that is costly to fake early or forge late.

The work adds a second physical commitment. The making holds a region of memory, and each step needs the one before it. Every step reads a full 16 KiB slot of a 256 MiB terrain. The design aims to leave no cheap way to skip ahead or split the route. [Why memory, and why a clock?](why-memory-and-why-a-clock.md) Reading a tick into the work fixes its early end. Later ticks cut the stretch along the way. The work sits between public moments. It cannot be done before the tick it names, and a finished file cannot claim a tick it did not read.

The block supplies the when. The memory supplies the what. Neither is new on its own. The pairing is what this project tests, on commodity hardware, at the scale Bitcoin already runs.

This does not prove a person, and it does not set a price. What a holder accepts is a gate the holder keeps.
