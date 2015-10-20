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

  1. Idempotency: For any *s* &isin; *S*,
     *m*(*s*, *s*) = *s*.
  2. Commutativity: For any *s*, *t* &isin; *S*,
     *m*(*s*, *t*) = *m*(*t*, *s*)
  3. Associativity: For some *r*, *s*, *t* &isin; *S*,
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

Fortunately, if we wish to prove a model *M*<sub>1</sub> = (*S*<sub>1</sub>,
*m*<sub>1</sub>) is consistent, we can choose a model *M*<sub>2</sub> =
(*S*<sub>2</sub>, *m*<sub>2</sub>) which is known to be consistent and a
surjective transformation function *T* : *S*<sub>2</sub> &rarr; *S*<sub>1</sub>
and show that *m*<sub>1</sub>(*T*(*s*), *T*(*t*)) = *T*(*m*<sub>2</sub>(*s*,
*t*)) holds for any *s*, *t* &isin; *S*<sub>2</sub>.

This proof is nasty, so don't read it if you don't truly care about why the
above works. We can prove that this implies the consistency of *M*<sub>1</sub>
by showing that each of the 3 conditions for consistency *m*<sub>2</sub> is
required to meet imply the same condition on *m*<sub>1</sub>. Since
*m*<sub>1</sub> meets all three criteria, *M*<sub>1</sub> is consistent.

 1. Idempotency:
    * *m*<sub>2</sub>(*x*, *x*) = *x*
      &nbsp;&nbsp;&nbsp;for any *x* &isin; *S*<sub>2</sub>
    * *T*(*m*<sub>2</sub>(*x*, *x*)) = *T*(*x*)
    * *m*<sub>1</sub>(*T*(*x*), *T*(*x*)) = *T*(*x*)
    * *m*<sub>1</sub>(*y*, *y*) = *y*
      &nbsp;&nbsp;&nbsp;for any *y* &isin; *S*<sub>1</sub> (by surjectivity)
    * Therefore, *m*<sub>1</sub> is idempotent

 2. Commutativity
    * *m*<sub>2</sub>(*a*, *b*) = *m*<sub>2</sub>(*b*, *a*)
      &nbsp;&nbsp;&nbsp;for any *a*, *b*
      &isin; *S*<sub>2</sub>
    * *T*(*m*<sub>2</sub>(*a*, *b*) = *T*(*m*<sub>2</sub>(*b*, *a*))
    * *m*<sub>1</sub>(*T*(*a*), *T*(*b*)) = *m*<sub>1</sub>(*T*(*b*), *T*(*a*))
    * *m*<sub>1</sub>(*c*, *d*) = *m*<sub>1</sub>(*d*, *c*)
      &nbsp;&nbsp;&nbsp;for any *c*, *d* &isin; *S*<sub>1</sub> (by
      surjectivity)
    * Therefore, *m*<sub>1</sub> is commutative

 3. Associativity
    * *m*<sub>2</sub>(*r*, *m*<sub>2</sub>(*s*, *t*)) =
      *m*<sub>2</sub>(*m*<sub>2</sub>(*r*, *s*), *t*)
      &nbsp;&nbsp;&nbsp;for any *r*, *s*, *t* &isin; *S*<sub>2</sub>
    * *T*(*m*<sub>2</sub>(*r*, *m*<sub>2</sub>(*s*, *t*)) =
      *T*(*m*<sub>2</sub>(*m*<sub>2</sub>(*r*, *s*), *t*))
    * *m*<sub>1</sub>(*T*(*r*), *T*(*m*<sub>2</sub>(*s*, *t*))) =
      *m*<sub>1</sub>(*T*(*m*<sub>2</sub>(*r*, *s*)), *T*(*t*))
    * *m*<sub>1</sub>(*T*(*r*), *m*<sub>1</sub>(*T*(*s*), *T*(*t*))) =
      *m*<sub>1</sub>(*m*<sub>1</sub>(*T*(*r*), *T*(*s*)), *T*(*t*))
    * *m*<sub>1</sub>(*x*, *m*<sub>1</sub>(*y*, *z*)) =
      *m*<sub>1</sub>(*m*<sub>1</sub>(*x*, *y*), *z*)
      &nbsp;&nbsp;&nbsp;for any *x*, *y*, *z* &isin; *S*<sub>1</sub>,
      by surjectivity.
    * Therefore, *m*<sub>1</sub> is associative

Don't worry if your eyes glossed over during that last part. The Markdown
source is even less readable. It's just some substitutions to show that the 3
consistency criteria on *m*<sub>2</sub> and the equivalence defined above lead
to the 3 criteria holding for *m*<sub>1</sub> as well. I've included the proof
here for the sake of completeness, so that readers can choose to verify my
conclusion if they wish.
