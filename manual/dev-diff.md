% State changes

Since IRC is a state transition protocol, we need to be able to detect state
changes and react to them.

## The `Diffable` trait

The `Diffable` trait should be implemented on any piece of state that a client
may care about. When a type implements `Diffable`, it's claiming that it can
look at two instances of itself and decide what has been added, removed, or
changed between revisions.

## Protocol module responsibility

A protocol module is generally responsible for taking state changes and
relaying them to clients in the form of IRC messages. When something about the
IRCD state changes, the core will send the protocol module references to the
old and new states and allow it to analyze them to determine how to send
updates to clients.

This could become inefficient, so I'm working on a way to cleanly allow the
core to send protocol modules the smaller pieces of state that have changed.
