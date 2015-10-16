% Registering

There are three separate concepts to sort out: your connection, your identity,
and your nicknames.

Your **connection** is just what it sounds like. Each connection, from connect
to disconnect, constitutes a single connection.

Your **identity** is a more abstract concept. You can protect your identity
with a password or some other form of authentication. Your identity lives past
the end of your connection, and is the primary owner of "your" things. This is
a bit of a subtlety in contrast with traditional IRC. Nicknames, channels,
statuses, etc. are no longer attached to a single connection but to an
identity.  You can assume the role of an identity on connect by providing the
correct credentials. All connections have an associated identity; `ircd-oxide`
creates a temporary identity for connections that haven't yet associated with a
concrete one. Temporary identities expire immediately on disconnect and cannot
be explicitly authenticated to. Registered identities can take months to expire
after the last connection using it ends.

Your **nicknames** are simply an asset owned by your identity. Again, this is a
subtlety in contrast with more common IRC implementations. Nicknames are not
owned by connections but by identities, which outlive connections! Implicit
claims to nicknames, such as those created by temporary identities or by the
`/NICK` command, expire within a few minutes after their last use. Explicit
claims, such as registration and the `/CLAIM` command, can take up to weeks to
expire. Note that the primary nickname for an identity will never expire before
the identity expires.
