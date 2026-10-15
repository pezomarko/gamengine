# Contributing

Small rules that keep a fast codebase fast to change. They apply to the director, to the
AI that does most of the engineering, and to anyone else.

## The compiler

`rust-toolchain.toml` names the one compiler everybody builds with; rustup installs it on
the first `cargo` call. CI uses the same version, so a lint that a newer clippy adds never
turns `main` red on its own. Moving the pin is a commit of its own, with the lints the new
release adds fixed in the same commit.

## Green main

`main` is green or it is being fixed. The fast lane (`.github/workflows/ci.yml`: format,
clippy for the native and the browser targets, unused dependencies, the tests, the asset
budgets, the committed content bundle reproduced, the binary and the browser sizes) is what
a pull request waits for, and it finishes in minutes. A red fast lane after a merge is
fixed or reverted the same hour, before any other work lands.

The long gates (`.github/workflows/gates.yml`: bots over the wire, the screens on a display,
the browser in headless Chromium, the render and the sound on a software GPU) run nightly
and on demand, one job each with its own clock, so a hang in one hides nothing. A red gate
is a bug to fix or a gate to repair, in a commit of its own, the same day. Locally the same
gates are `scripts/check-*.sh`; never run a gate and the test suite at the same time.

## Commits and pull requests

One change per commit, one topic per pull request, and a pull request a reviewer can read
in under an hour: a few hundred changed lines, not a phase. The subject line is
`type(scope): what changed`, at most 72 characters, where `type` is one of `feat`, `fix`,
`refactor`, `perf`, `test`, `docs`, `build`, `ci`, `chore`, and `scope` the crate or the
area (`client`, `zone`, `hub`, `content`, `net`, `model`, `ci`). The body says why and what
was measured, in sentences; a quote from the director goes there, not in the subject.
Decisions about the design go in the contract document the change implements (`docs/*.md`),
dated, with what was rejected and why; the commit message points at the section.

```
feat(client): composite cloth armour into the body's atlas

The cuirass and the trousers were skinned overlays with a 512² texture and a draw
call each (LOOK.md 6.6). They are patches on the body's own atlas now ...
```

## Functions and files

Clippy's thresholds are the ceiling: a function under 100 lines, a type a reader can hold
in their head, a file one can scroll. A frame loop or a connection handler that has grown
past that is split by what it does (input, simulation, draw lists, the screens) before
the next feature lands in it, not after.

## Dependency direction

`gm-core` knows no I/O; the client, the zone, the hub and the bot link what they need and
nothing a sibling needs; `gm-ai` is the minds (the zone, the bot) and `gm-nav` the grid
(everybody). `cargo machete` runs in the fast lane: a crate nobody links is removed, not
kept for later.

## Worktrees and build caches

Every git worktree keeps its own `target/`; four of them filled a 426 GB disk once. On a
shared machine set `RUSTC_WRAPPER=sccache` (`~/.cargo/config.toml`) so the dependencies are
compiled once for every worktree, and delete `target/debug` of a worktree you are not in:
it rebuilds in minutes. Check `df -h` before a long chain.
