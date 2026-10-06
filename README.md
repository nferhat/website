# `website` - An opinionated website builder

This is `website` (yes I know, amazing name). An opinionated SSG. In short, you give it some [djot](https://djot.net) content, and it will 
automatically do the following:

- Read out the whole structure of your content
- Compile the djot contents (`.dj`/`.djot` files) into valid HTML
- Compile styling using [SASS](https://sass-lang.com)
- Bundle everything and make it ready for production in a `dist/` folder.

This is comparable to already existing tools, like [Hugo](https://gohugo.io) or 
[Zola](https://getzola.org). And while yes, they could have served me with enough 
tweaking, however the fact I had to do tweaking, changes and more justified me 
not wanting to bother with those and instead roll out something of mine

## Opinionated

This means that everything here is of my choice. Tradeoffs have been made to suit me better, and you
might not like those. Some of which include

- Non flexible website content layout
- Auto-hashes and caches assets by content-hash
- Lack of extensive configuration
- Uses tree-sitter for highlighting code blocks
- Forced use of [SASS](https://sass-lang.org) for styling
- [Djot](https://djot.net) over markdown. Each page starts with a `+++` delimited TOML frontmatter.
- [Liquid](liquid-engine) for templating

---


## Should I use this?

Short answer: *no*. This is solely meant for <https://nferhat.dev>, IE my personal website. 
You are better off using the solutions cited above (or the many more that are designed for 
other people to use)

It works fine *for me*, but I cannot assure you it will work fine for you. I won't assure the fact
that you'll be able to understand anything happening here, since I probably won't be documenting
this.

However, I still leave it open source for reference.

---

## TODO

- [x] Rendering of djot files 
- [x] Proper templating using Shopify's `liquid` engine
  - See `compiler/src/templates/context.rs` for passed-in variables
  - Supports `partials`! You can seamlessly use the `render` directive.
- [x] Codeblock highlighting using tree-sitter 
- [x] More cohesive website generator.
   - ~~For now we only have single-page generators. However, for some pages, we might need to get a
   "run over all pages" state, for example a "All blogs" page, for example.~~ There's the new `compiler::Compiler`
   structure handling this
- [ ] Incremental builds 
   - `cli serve` supports serving, but it's hot-reloading is kinda iffy, even though it only recompiles
   the page you modified, this might not be it for the long run. This kinda ties in with the previous point

[liquid-engine]: https://shopify.dev/docs/api/liquid/
