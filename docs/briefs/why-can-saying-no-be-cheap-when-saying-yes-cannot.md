# Why is the check always cheaper than the making?

Every depth of check costs the verifier less than the making cost the producer.

The last page put the evidence in the file. [Can you check a stranger without asking anyone?](can-you-check-a-stranger-without-asking.md) This page is what that check costs, and what it can say.

There is a familiar idea in cryptography: some things are hard to make and easy to check. A signature takes real effort to forge and almost none to verify. The same inequality holds here, and it holds at every depth. A check is not a second making. Time, energy, and the resources of the machine stay on the verifier's side of it.

A cheap fake shows a mismatch soonest. A wrong length, a digest that does not match the bytes it claims, or a mark that does not sit where it says in the public sequence: those show a mismatch before any deeper look. [How do you prove when the work happened?](how-do-you-prove-when-the-work-happened.md) A genuine object would have matched them on the way in, so one mismatch is enough to report that this object does not match. That report says almost nothing else. It does not mean the object is fake in some deeper sense. It means this one did not survive an inspection a genuine object would have matched.

A fancier fake survives that and asks for a closer look. The closer look costs more than the first inspection. It is still cheap, and it is still cheaper than making the fake.

A sophisticated object, or one whose characteristics look suspicious, asks for a little more again. That look is the fullest check on this page: enough of the work, taken again, to establish the results the object claims. It costs more than the closer look. It is still much cheaper than producing that object. The inequality does not flip because the object was elaborate.

A shallow inspection can show a mismatch. It cannot confirm. A match on the cheap inspections still leaves the work unconfirmed. Confirming means the fuller look. That look is not the producer's bill, and it is not a toll on arrival. It runs on something that already looks real, because everything cheaper has had its chance to show a mismatch first.

What the fuller look establishes is a measurement: this object is what that sequence of work produces. The check measures the object. Whether to accept it is the reader's policy about whether that fact is enough. Neither substitutes for the other.
