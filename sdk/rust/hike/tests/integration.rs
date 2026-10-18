//! Producer, mock Bari, and the tip sequence.
//!
//! The drive is `hike::produce`. Observe is the tip-sequence argument. The
//! stage-open hook posts to the in-process Bari and asks for the path.
//! Rules A1.1 and A1.2 are the scripted small-profile drive in `produce`.
//! Rule A1.4 is the Hashcash and waypoint fault coverage in `http`.
