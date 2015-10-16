% Monitoring

`ircd-oxide` has basic monitoring support to detect abnormalities. Some of the
metrics collected are as follows:

  * Process metrics
    * Memory usage
      * `proc.vsz`, Virtual memory size
      * `proc.rss`, Resident stack set
    * CPU usage
      * `proc.pcpu`, Percent cpu
      * `proc.load`, Load average
    * `proc.thread`, Thread count
    * Network usage
      * `proc.netin`, Incoming bytes
      * `proc.netout`, Outgoing bytes
  * Profiling metrics
  * Application metrics
    * Local client command handling time
      * `app.cmd.*`, All commands
      * `app.cmd.PRIVMSG`, `PRIVMSG` command
      * etc.
    * Connection count
      * `app.conns.*`, Global connection count
      * `app.conns.<sid>`, Connection count for server `<sid>`
    * `app.chans`, Channel count
    * `app.idents`, Identity count
    * `app.nicks`, Nickname count

Logs can be queried from within `ircd-oxide` as well, or sent to a channel for
constant monitoring. Log messages are divided up into hierarchical categories
to make it easier to extract information. Some of the logging categories
follow:

  * Gotta think about this some more

## The `/MET` command

> *A more detailed summary of this command can be found in the operator command
> reference appendix at the end of this book.*

The `/MET` command is used for querying metrics. The basic format is as
follows:

```plain
/MET [options] [metric ...]

If no metric is supplied, /MET defaults to printing ALL aggregate metrics. For
example, 'app.cmd.*' is considered an aggregate metric, while 'app.cmd.JOIN' is
not.

Options
  -1, -5, -15
          Summarize metrics in 1, 5, or 15 minute periods. Defaults to 1. Only
          one of these may be supplied.
  -t N    Show the last N periods.
  --format=FORMAT
          Output in the specified format. FORMAT can be either 'short' (the
          default), or 'tall' for larger graphs.
  -avg    Print the average value of each metric in the given period.
  -sum    Print the sum of the data points in the given period.
  -n      Print the number of data points in each period.
  -pNN    Print the NNth percentile for the metric in each period. Acceptable
          values for N are 0 (min), 50 (median), 90, 99, 100 (max),
```
