# Contributing

fakeobd is a lab simulator. A change should keep the same profile and the same transcripts producing the
same bytes, and should arrive with a test when it changes behaviour. Read [AGENTS.md](AGENTS.md) and
[docs/SCOPE.md](docs/SCOPE.md) first: some things are deliberately not built.

## Before a pull request

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p fakeobd -- verify transcripts/*.txt
python tools/break_check.py
```

- A new `[ok]` row in ROADMAP.md needs a test that names it and a mutation in `tools/break_check.py` that
  makes that test fail.
- A behaviour that depends on a standard or datasheet nobody has read is `[unread]` and gets a gap entry.
- Do not regenerate a transcript blindly; read every changed reply.
- Do not commit `.env`, audit logs or captures from a real adapter, vehicle or phone.

## License

MIT OR Apache-2.0. Contributions are dual-licensed the same way ([LICENSE-MIT](LICENSE-MIT),
[LICENSE-APACHE](LICENSE-APACHE)).
