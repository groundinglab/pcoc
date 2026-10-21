# Why memory, and why a clock?

Occupancy is the unit. The time half is not in the bytes.

The cost that will not divide is a holding. [What kind of cost refuses to divide?](what-kind-of-cost-refuses-to-divide.md) That leaves two questions, in order. Why is the holding memory? And why is a clock in the same sentence?

Memory first. Fetching a piece of memory is quick. Most of the time goes to the processor doing arithmetic. The reason to hold a region is what the arithmetic is arranged to need. Every step reads a full 16 KiB slot of a 256 MiB terrain. The step mixes the walk state into every word of that slot. A digest, a lookup table, or a smaller read cannot stand in for the slot. The next step needs the state this step produced. There is no looking ahead. This is not a search for a rare result. The work is the forced read of the whole slot, from a region you have to keep holding.

A producer who stores nothing, or who stores on disk, pays more than one who holds the region in memory. The design did not impose the memory. It arranged the work so that memory is what anyone sensible would choose.

What is being spent is occupancy: space held for a length of time. The making keeps that region resident while it runs. The space is held for the stretch, not only for the moments of arithmetic.

The unit is familiar. Nobody rents a room. They rent a room-night. Two guests cannot occupy the same room on the same night. Ten nights costs ten nights whether one traveller takes them in a row or ten take them at once. Each making holds its own region for its own duration.

A room-night has a night in it. That is why a clock is in the same sentence. The holding is not a pause after the work. The making keeps the region resident the whole way through, and a public interruption cuts the stretch. The region is not handed back at that cut, because the making is not finished. Releasing it would abandon unfinished work. The clock is the interruption. It is not a proof sitting in the file. It is time on a public clock nobody owns.

The space half leaves evidence. There is an object at the end, and checking it re-runs enough of the work to establish the results the object claims. The time half leaves nothing in the bytes alone. A producer can hand over a result and say it took a day. Nothing in the file distinguishes a day from a decade. Duration is the half the person claiming it cannot demonstrate.

What supplies that when is [How do you prove when the work happened?](how-do-you-prove-when-the-work-happened.md)
