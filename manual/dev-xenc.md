% The XENC format

XENC is an octet-based serialization format used in a handful of places around
`ircd-oxide`, notably in Oxen, the cluster protocol. XENC is based on Bencode,

XENC supports the following types of data:

  * Integers
  * Timestamps
  * Octet strings
  * Lists, whose values may be any XENC value
  * Dictionaries, which map byte array keys to XENC values

It is comparable to JSON in functionality.

## Grammar

As an example, consider the following JSON:

```json
{
  "action": "added",
  "members": [
    {
      "login": "octocat",
      "id": 583231
    }
  ],
  "repository": {
    "id": 35129377,
    "name": "public-repo"
  },
  "sender": {
    "login": "baxterthehacker",
    "id": 6752317
  }
}
```

When encoded as XENC, this would look like the following, excluding newlines:

```plain
d6:action5:added7:membersld5:login7:octocat2:idi583231eee10:repositoryd2:idi351
29377e4:name11:public-repoe6:senderd5:login15:baxterthehacker2:idi6752317eee
```

Significantly less readable, but compare to the non-prettified JSON:

```json
{"action":"added","members":[{"login":"octocat","id":583231}],"repository":{"id
":35129377,"name":"public-repo"},"sender":{"login":"baxterthehacker","id":67523
17}}
```

XENC can be prettified as well, although it becomes invalid in the process:

```plain
d
  6:action 5:added
  7:members l
    d
      5:login 7:octocat
      2:id i583231e
    e
  e
  10:repository d
    2:id i35129377e
    4:name 11:public-repo
  e
  6:sender d
    5:login 15:baxterthehacker
    2:id i6752317e
  e
e
```

## Concrete API

TODO
