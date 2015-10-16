% Configuration

The `ircd-oxide.toml` configuration file is very lightweight, containing only
what the server needs to connect to the cluster. Most configuration is managed
at run time, through the `/CONF` command.

The basic procedure to change some configuration is as follows:

  * Use `/CONF SET` to find settings and stage changes.
  * Use `/CONF COMMIT` to save all changes at once.
  * Or, use `/CONF CANCEL` to cancel a set of changes.

In the event of a conflict, such as on netjoin, the most recent ***full set of
changes*** is used. That is, from a consistency point of view, the entire
configuration is seen as one atomic item.
