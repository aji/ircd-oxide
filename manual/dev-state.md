% Reaching consensus

To portray a single abstract "network" with a cluster of independent servers,
`ircd-oxide` nodes need to be able to collectively agree on globally shared
state at some level. This topic is called "distributed consensus" and has been
studied quite extensively in both academic and practical contexts.

## Eventual consensus

`ircd-oxide` takes a relatively simple approach to distributed consensus in
that every node is allowed to make its own decisions independently with the
understanding that the decision can be superseded at any time by a decision
made by any other node, however *all* nodes will eventually agree on what the
actual decision was. In this way, it is a sort of eventual consensus, which
happens to be good enough for our use case.

Eventual consensus is accomplished by requiring state to be mergeable. A
stateful type must have an `identity` value and a `merge` operation. If a piece
of state were a set, these operations would be analogous to Ø, the empty set,
and ∪, set union. All state changes are implemented as `merge` operations, and
servers communicate a state change by broadcasting the new state in its
entirety. *As long as all servers see all new state, at any point in time, in
any order, with any duplicates, they will come to the same conclusion for the
final state.*

### Proof of consensus

We will expand on this analogy and prove that this model lets us eventually
reach consensus. Suppose our piece of state is a set of arbitrary objects and
we can only construct new sets and take unions of sets. At some arbitrary point
in time, suppose all nodes agree that the current state of the set is *S* = {
*A*, *B*, *C* }. Then suppose that two nodes decide to update this state. One
wishes *X* to be included in the set, the other *Y*. The first node takes the
union of *S* and { *X* }, and obtains *S<sub>x</sub>* = { *A*, *B*, *C*, *X* }.
The second node similarly obtains *S<sub>y</sub>* = { *A*, *B*, *C*, *Y* }.
Both nodes broadcast the new sets. Suppose a node receives *S<sub>x</sub>*
first. They will take the union of their stored state *S* and the incoming
state *S<sub>x</sub>* and arrive at *S<sub>x</sub>*. When *S<sub>y</sub>*
arrives, they will take the union of their stored state *S<sub>x</sub>* and the
incoming state *S<sub>y</sub>* and arrive at a new set *S<sub>xy</sub>* = {
*A*, *B*, *C*, *X*, *Y* }. If *S<sub>y</sub>* were to arrive before
*S<sub>x</sub>*, the node would arrive at the same result.  It's also clear
that if a message were to arrive more than once, the final result would not
change. Therefore, with this model, all nodes will eventually agree that
*S<sub>xy</sub>* is the current state of the data.

Seeing as we can achieve consistency by using sets as state and performing set
union for updates, we can map this model to IRC. Suppose we store channel
topics as sets of every value the topic has had over time. By the above proof,
this would clearly allow all servers to agree on the final state of the set of
topics. However, we do not usually think of a channel topic as being a set of
possible values. A channel only ever has one topic!

We can get around this issue by requiring servers to look at a set of possible
topics and to pick, independently of all other factors, what the *true* topic
is. As long as every server has the same set of possible topics, they will all
agree on the true topic. How this choice is implemented could be completely
arbitrary. Servers could pick the topic that comes first lexicographically, or
the topic that has the most occurrences of the letter 'a' breaking ties by
counting the number of 'b', etc. The exact critera is not important, as long as
it can be applied consistently by all servers.

While arbitrary criteria would work, it's important for usability that the
criteria have some connection to "fairness". For channel topics, the measure
that is commonly used is the time the topic was set as reported by the host
making the change. While it's true that this doesn't accurately reflect the
"true" ordering of events in real world time, it's often considered "good
enough". Topics don't change often enough for it to be a problem.

> <span style="font-size:small;font-style:italic">
> As an aside, it should be pointed out that the TS6 protocol puts rather
> complicated semantics on topic timestamps.  Timestamps presented during
> netjoin are handled specially, giving the *older* topic precedence. This is
> to prevent abuse during netsplits where users on a small enough half of the
> split can take control of the channel and change the topic maliciously.  Due
> to `ircd-oxide`'s channel management principles, however, this sort of
> misbehavior is not possible, so we can safely take the newest topic in all
> cases.</span>

So far, our "topic state" is actually a set of all updated topics and the time
they were set. When choosing the topic to present to users, the server looks
for the topic with the newest timestamp. However, since storing and
transmitting the complete history of topics can be problematic, we can actually
get away with only storing and transmitting the newest topic. Consider that the
act of taking the union of two topic histories and picking the newest item has
the same effect as taking the newest item from two topic histories and picking
the newer one. Reread that sentence carefully! As a result of this, we can get
away with only storing and transmitting the newest topic we've ever seen for a
channel, and performing a "merge" by picking the newer of the two topics.
