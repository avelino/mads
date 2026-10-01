---
paths:
  - "docs/**"
  - "README.md"
  - "CONTRIBUTING.md"
---

# Documentation rules

- Accuracy over volume. The code and its tests win over the spec. Quote real output only: run the binary (`cargo build -q -p mads-cli`, `target/debug/mads ...`). Do not run paid providers to get a sample.
- Never document a flag, default, key, code or message you did not check in the code.
- Invoke the `voice-humanize` skill before writing new prose. Short sentences, consequence first, no hedging.
- No em dashes (U+2014), no semicolons chaining thoughts, no colons for emphasis. No inflated words: comprehensive, crucial, robust, seamless, leverage, streamline, fundamental, powerful.
- Docs are English. Headings in sentence case. Every code fence has a language. Every page starts with an H1 and one sentence saying what the reader will be able to do.
- Docs are a GitBook: a new page goes into `docs/SUMMARY.md`. Use relative links.
- `docs/superpowers/` holds design specs. Do not rewrite them to match the code: append to section 16 of the v1 spec instead.
- When behavior changes, grep the docs for the old sentence (`rg "old text" docs README.md`) and fix every page, not only the one you were editing.
