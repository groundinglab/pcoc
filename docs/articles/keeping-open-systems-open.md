# Keeping Open Systems Open Without Giving Control Away

Bots and AI agents can now make thousands of identifiers in the time it takes you to make one.

Open systems are valuable because they stay open. Anyone can create an identifier and start using the system without asking permission. That low barrier is the point. It is also the problem. When creating additional identifiers costs almost nothing, bots and farms can generate as many as they need. Openness becomes an attack surface. An open system still has to work when many of its identifiers are fake.

Christopher Allen's 2016 principles, [The Path to Self-Sovereign Identity](https://www.lifewithalacrity.com/article/the-path-to-self-soverereign-identity/), set out what must not be given away: the individual remains the ultimate authority over their own identity, consents to its use, and discloses only what they choose. Those principles still hold, but they do not put a cost on making extra identifiers.

When open systems face large numbers of low-cost identifiers, the common responses move the decision of who may participate away from the individual. An issuer decides which identifiers are valid. A biometric check requires the user to submit a physical measurement to an external service. A social-graph method scores accounts according to connections the scoring system observes. A detection system continuously updates rules the individual neither controls nor fully sees. In each case the system gains a way to reduce bots. In each case the individual loses a measure of control over the identifier itself. The cost of an extra identifier stops being something the maker pays. It becomes a condition someone else sets.

The question is whether the cost of each additional identifier can be raised without moving the decision away from the individual.

A resource commitment performed by the maker raises that cost without creating a new issuer. The maker spends a scarce local resource, time and occupancy of memory, to produce a small file. Anyone can check the file cheaply and independently. The check measures whether the file was made the way the specification requires, at the pace it requires. It discloses no personal data. It requires no trust in the maker, and any index it reads can be rebuilt from Bitcoin itself. Because the cost sits in the making of the object, producing many identifiers requires repeating the commitment many times. The cost is memory held across Bitcoin blocks, so it can't be bought faster with more hashrate, and it isn't a payment to anyone. The maker keeps control of the identifier. Each relying system keeps its own policy for whether a given commitment is enough for admission.

We have published a specification for an interoperable resource commitment of this kind. The specification can be bound to other credential systems as a rate-limiting component on the making of identifiers. A PCOC is an object made under that specification. It is self-issued: the maker makes it alone, without asking anyone. Making one holds memory for a few hours. It takes a resource commitment, not a payment. Its stages are paced by Bitcoin blocks, used only as a shared public clock. The result is a small file that anyone can verify, and verifying is far cheaper than making. We provide reference implementations and a working demonstration on Nostr. Each service that accepts the file still sets its own admission policy.

Nostr is the first test: a large, permissionless system. If the approach holds there, other open systems with the same constraints can use it.

## Open questions

- The economics of large-scale farms are not yet settled.
- Individual systems will choose different policies for when and how to require a commitment.
- The relationship between this resource-commitment approach and other uniqueness mechanisms will depend on the specific threats open systems face.

Read the specification at [groundinglab.io](https://groundinglab.io/technical/spec/), or see the code at [github.com/groundinglab/pcoc](https://github.com/groundinglab/pcoc).
