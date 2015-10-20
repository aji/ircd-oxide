% Reaching consensus

To portray a single abstract "network" with a cluster of independent servers,
`ircd-oxide` nodes need to be able to collectively agree on globally shared
state at some level. This topic is called "distributed consensus" and has been
studied quite extensively in both academic and practical contexts.

Within `ircd-oxide`, a number of terms are used when discussing state, which we
define here. A *stateful* thing is any thing that can change over time in
response to events, in particular something that all nodes in the cluster must
be aware of. A *state object* is the in-memory representation of some stateful
thing. For a given stateful thing, the corresponding state object should
contain the necessary information to determine the true state. A *merge* is the
action taken to reconcile diverged state objects into a new state object. For
simplicity's sake, all state changes are modeled as merges. The *state model*
is the collective name for the set of possible state objects, the associated
merge operation, and the function to determine the true state for some state
object, if defined.

## Consistency model

`ircd-oxide` takes a relatively simple approach to distributed consensus in
that every node is allowed to make its own decisions independently with the
understanding that the decision can be superseded at any time by a decision
made by any other node, however *all* nodes will eventually agree on what the
actual decision was. This is weaker than the typical use of the term
"consensus", but is sufficient for our use case. In the context of CAP theorem,
we are giving up strong consistency in favor of availability and partition
tolerance.

### Consistency, as we define it

The goal then is to define, for each stateful thing, a *state model* that is
consistent. We consider a state model *consistent* if its merge operation is
idempotent, commutative, and associative. More formally, a merge function *m* :
*S* &times; *S* &rarr; *S* for some set of possible state objects *S* should
satisfy all of the following conditions:

  *  Idempotency: For any *s* &isin; *S*,
     *m*(*s*, *s*) = *s*.
  *  Commutativity: For any *s*, *t* &isin; *S*,
     *m*(*s*, *t*) = *m*(*t*, *s*)
  *  Associativity: For some *r*, *s*, *t* &isin; *S*,
     *m*(*m*(*r*, *s*), *t*) = *m*(*r*, *m*(*s*, *t*))

With these conditions, we are guaranteed that if any two nodes have merged the
same set of state objects, in any order, with any duplicates, they will end up
with the same state object. This is an important conclusion! Informally, if
node A sees messages 1 2 3 and node B sees messages 3 1 2 1, then A and B will
still agree on the final outcome of processing those messages.

Note that if a node has yet to receive some message, its opinion of what the
true state is will be outdated in an undetectable way! This is why our model is
merely *eventually* consistent, not *strongly* consistent. However, it allows
us to be partition tolerant while still letting users make changes to the
system. When the two halves of a partition are healed, nodes will exchange and
merge state objects and once again reach a consistent state.

### A simple, consistent state model

We construct now a simple state model that has these properties. Let *I* be
some arbitrary "information set" whose elements are atomic pieces of
information. We define *S* = *P*(*I*) to be our set of state objects, where *P*
denotes the power set operation. In other words, a state object in this model
is some finite subset of *I*, the set of possible information. The merge
operation *m* is defined as *m*(*a*, *b*) = *a* &cup; *b*. Clearly, *m*
satisfies all our conditions for consistency. Therefore, our state model is
consistent.

Consider further, for this state model, a function *t* : *S* &rarr; *X* that
determines, for a given finite subset of *I*, the "true state" *x* &isin; *X*.
Observe that *t*(*s*) for any state object *s* will have the same consistency
properties as *s* itself.

### A construction for consistent state models

Therefore, we can choose

  1. an information set *I*,
  2. an arbitrary set *X*,
  3. and a true state function *t* : *P*(*I*) &rarr; *X*, where *P* denotes the
     power set,

and construct a state model that is consistent for elements of *X*, using
subsets of *I* to trigger selection of new elements of *X*.

### Slimming it down

Sending around subsets of *I* and merging by set union, while theoretically
sound, is simply impractical. However, this model gives us a framework with
which to prove the consistency of more practical models.

As a simple example, suppose all servers must eventually agree on some element
of a set *X*. We choose our information set *I* to be pairs (*c*, *x*) where
*c* is a clock (a timestamp such that no two servers can generate the same
timestamp) and *x* is an element of *X*. Our true state function *t* looks at a
set of pairs and picks the element *x* from the pair with the greatest *c*.  It
is sufficient for the state object to be such a pair (*c*, *x*), and perform
merges by picking the pair with the greatest *c*. Consider two state objects
*s1* and *s2* under the set-based model, i.e. *s1* and *s2* are sets of pairs.
Observe that computing *t*(*s1* &cup; *s2*) is equivalent to computing *t*(*s1*)
and *t*(*s2*) and picking the pair with the greatest *c*. Therefore, it is
sufficient to store only the pair *t*(*s1*).

> Aside: I have not yet really developed a solid proofwriting tool for deriving
> a model's consistency from another's. I'm thinking the basic idea will be to
> take a known consistent state model *M1* = (*S1*, *m1*) and the state model
> whose consistency is being proven *M2* = (*S2*, *m2*) and define a function
> *f* from *S1* to *S2*. At this point, if it can be shown that *f*(*m1*(*a*,
> *b*)) equals *m2*(*f*(*a*), *f*(*b*)) for arbitrary *a*, *b* &isin; *S1*,
> then *M2* is consistent.
