% IRC basics

The rest of this chapter assumes a passing familiarity with IRC. If you are
already a somewhat capable IRC user, you can skip this section. Otherwise, read
on to learn the basic concepts of IRC. This section is provided as a courtesy,
and for the sake of completeness.

## Networks

An IRC **network** is a collection of one or more servers that create a single
IRC experience. At its inception, IRC was envisioned as one global network.
However, various forces led to *many* networks existing, each with their own
idiosyncrasies. Connecting to any server in a network gives you access to the
entire network. Certain network users have an administrative role and are
called "IRC operators" or "IRCops".

## Users and nicknames

When connecting to a network as a user, you choose a nickname. Nicknames are
the primary way people on the network will refer to you, and must be unique.
Traditionally, your claim to a nickname is lost the moment you disconnect.
Needless to say, this ends up being inadequate in cases where identity security
is deemed important, and so some networks have implemented solutions that allow
users to put passwords on nicknames. Other networks, however, have staunchly
refused to provide any sort of nickname ownership, so nickname ownership policy
is just one of the many ways networks can vary.

## Channels

While any two users can exchange private messages, channels allow multiple
users to communicate as a group. Channels have names beginning with the `#`
character, for example `#ircd-oxide`. Some networks have other kinds of
channels in addition to `#`-channels that use different prefixes, however these
are not common. Users can join channels, leave (part) channels, send messages
to channels, etc. Channels have a small amount of metadata, such as a topic,
which is arbitrary text, and mode strings, which control the basic behavior of
a channel. Some users on the channel may have channel operator status,
sometimes referred to as "chanop" or simply "op", and can access privileged
functionality, such as changing the topic, kicking or banning users, and
granting or revoking channel operator status from other users.
