# What registering fakeobd needs

The workspace `PORTFOLIO.md` lists `fakeobd` as registered on 2026-11-21: an untiered, unsequenced Rust
workspace, `publish = false`, remote `b1tninja/fakeobd`, no dependency on `voltkit`. The repository is
still private and was not transferred. [ROADMAP.md](../ROADMAP.md) section 7 stays `[open]` because a
tier, a wave, and GitHub issues for the `[open]` rows are not assigned, and this repository does not
edit `PORTFOLIO.md` or projecthub. `$27` stays closed. `$34`–`$37` stay unimplemented.

For whoever owns the workspace root (`D:\code`), the remaining steps are:

1. **`PORTFOLIO.md`**: the section 3 row is already there (2026-11-21). Do not add a second one. It is untiered and unsequenced, `publish = false`, and it names no first-party dependency.
2. **projecthub**: `ROADMAP.md` uses the `Status | Obligation | Anchor | Test` grammar. `python -m projecthub sync --check` is run from the workspace root, not from this repository. The `[open]` rows can still want GitHub issues. `[unread]` rows are the standards that are not in `primarysource`. This repository does not run that sync.
3. **The workspace `AGENTS.md`**: whether it lists `fakeobd` is owned by the workspace root. Do not edit it from here.
4. **Git remote and CI**: the remote is `b1tninja/fakeobd`. `.github/workflows/ci.yml` runs fmt, clippy, tests,
   the golden `verify` and `break_check.py` on Ubuntu and Windows once pushed. The break check is slow
   (about 35 cargo runs); drop it from CI if that is not wanted.
5. **Cross-references to update elsewhere** (not done from here): volt `docs/AVD-OBD-BRIDGE.md` section 4
   (the fake helper) could point at `fakeobd serve` as the richer alternative; `gattmirror`'s ROADMAP row
   E4 ("the Voltage app in BLE mode runs its ELM327 init against the simulated adapter") needs an
   ELM327 endpoint behind its simulated adapter, which `fakeobd` can be.
6. **Shared hooks**: `core.hooksPath` at the workspace root applies to repositories that set it; this
   repository set none. Setting it to `projecthub/src/projecthub/shells/githooks` would give it the
   heading-loss and scratch-file gates.
