# Security policy

## Supported versions

mads is still at 0.x. Fixes land on `main` and ship in the next release and the `edge` container image. Older releases do not get backports.

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Report it privately through GitHub: open the [Security tab](https://github.com/avelino/mads/security) and click "Report a vulnerability", or go straight to <https://github.com/avelino/mads/security/advisories/new>.

Tell me what you found, the version or commit, and how to reproduce it. A proof of concept helps a lot. I will reply on the advisory, keep you posted while I work on a fix, and credit you when it is published, unless you prefer to stay anonymous.

## Scope

mads talks to model providers and to Google Ads with credentials you give it, and it reads files you point it at. Problems in that path count: a leaked API key or token, a prompt that makes mads write outside its output directory, a URL check that can be bypassed, or anything in the container image. Bugs in the providers themselves belong to them.
