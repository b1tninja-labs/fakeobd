# What registering fakeobd needs

`fakeobd` was created by a side session that was told not to register it, edit `PORTFOLIO.md`, or touch any
other repository. For whoever owns the workspace root (`D:\code`):

1. **`PORTFOLIO.md`**: add a row. Suggested: untiered Rust workspace like `gattmirror`; name `fakeobd`;
   one line "fake vehicle behind an ELM327/STN adapter: simulator, read-only policy library, conformance
   harness". Dependencies: none first-party (`volt`'s docs are the data source, not a build dependency).
   Relationship rows: companion of `fakescan`, `fakeprint`; consumer of nothing; consumed by `volt`'s
   `docs/AVD-OBD-BRIDGE.md` bring-up (an alternative to its Python fake) and, later, by a relay that links
   `crates/obd-policy`.
2. **projecthub**: the project has `ROADMAP.md` in the `Status | Obligation | Anchor | Test` grammar. Run
   `python -m projecthub sync --check` from the workspace root after adding it; expect the `[open]` rows
   (sections 3 to 7) to want GitHub issues. `[unread]` rows are the standards that are not in `primarysource`.
3. **The workspace `AGENTS.md`** lists sibling projects by name; add `fakeobd`.
4. **Git remote and CI**: the repository has no remote. `.github/workflows/ci.yml` runs fmt, clippy, tests,
   the golden `verify` and `break_check.py` on Ubuntu and Windows once pushed. The break check is slow
   (about 35 cargo runs); drop it from CI if that is not wanted.
5. **Cross-references to update elsewhere** (not done from here): volt `docs/AVD-OBD-BRIDGE.md` section 4
   (the fake helper) could point at `fakeobd serve` as the richer alternative; `gattmirror`'s ROADMAP row
   E4 ("the Voltage app in BLE mode runs its ELM327 init against the simulated adapter") needs an
   ELM327 endpoint behind its simulated adapter, which `fakeobd` can be.
6. **Shared hooks**: `core.hooksPath` at the workspace root applies to repositories that set it; this
   repository set none. Setting it to `projecthub/src/projecthub/shells/githooks` would give it the
   heading-loss and scratch-file gates.
