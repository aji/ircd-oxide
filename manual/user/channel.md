% Channel management

In a lot of ways, `ircd-oxide` channels function similarly to traditional IRC
channels. `ircd-oxide` channels...

  * ...start with `#`.
  * ...have normal users, voiced users, and operators.
  * ...have topics and modes.
  * ...can be made secret, invite-only, moderated, etc.

However, there are important differences that make `ircd-oxide` channels
unique:

  * Channels do not disappear after the last person leaves them.
  * Channel statuses (op, voice, etc.) are remembered after a person leaves the
    channel.
