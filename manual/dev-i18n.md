% Internationalization

Internationalization, or more commonly, i18n, is an important design principle
to make `ircd-oxide` accessible to users in their own language.

Here's a list of things that should be i18n-ready:

  * Success and error messages for commands
  * Help output

And a list of things that should be in English always:

  * Protocol strings (e.g. `JOIN`)
  * Log messages
  * Configuration files

The basic rule is that anything a human client connected to the network might
see should be i18n-ready, while anything that computers or administrators might
see is okay to be English-only.

# Using the `i18n` module

I haven't come up with the specifics of the design yet but it will probably
look something like this:

```rust
// somewhere distant...
let mut ctx = i18n::I18n.new();
ctx.load_translations(...);

// ctx is stored immutably somewhere

// somewhere you need translations
let t = ctx.translator("es");
// alternately,
// let mut t = ctx.default_translator();
// t.set_locale("es")

t.f_("Welcome to the {} IRC network", &["FriendlyChat"]);
// => "Bienvenidos al FriendlyChat IRC"

// more ergonomically,
f_!(t, "Welcome to the {} IRC network", "FriendlyChat");
```
