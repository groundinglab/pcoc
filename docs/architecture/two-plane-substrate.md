# Two-plane substrate

A plane is four parameters, one value. Produce, path fetch, verify, and admit are the same code. A run supplies the four together. The code does not choose a tip, a Bari, an index, or a policy.

| Parameter | Test | Live |
|---|---|---|
| Tip source | Synth sequencer, one adjustable cadence, mean about 60 seconds | Bitcoin selected-chain gossip |
| Bari (batching plant) | Test Bari, advanced by that synth tip | Live Bari, advanced by that Bitcoin gossip tip |
| Commitment surface | The index projected by test Bari | The index projected by live Bari |
| Admit policy | Test gate, test policy only | Live door, live policy only |

An object bound on one plane is not a candidate on the other. Verify measures the object; admit applies that plane's policy to the measurements. A verify measurement is not an admission decision.
