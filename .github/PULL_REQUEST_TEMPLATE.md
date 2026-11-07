## Summary

What does this OxidePress pull request do? (1-3 sentences)

## Crate

Which `oxidepress-*` crate from `crates.map.json`? Docs-only changes say `docs`.

## Changes

-

## Related Issues

Closes #

## Checklist

- [ ] `cargo check -p <package>` passes
- [ ] `cargo test -p <package> --lib` passes
- [ ] `cargo fmt -p <package> -- --check` passes
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace --lib --bins -- --skip e2e` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] Cookie `rustpress_session`, option keys `_rustpress_session_`, tables `rustpress_sessions` / `rustpress_cache` / `rustpress_migrations`, template key `rustpress_version`, env vars `RUSTPRESS_*`, WordPress `wp_*` tables, `themes/`, `static/`, `wp-content/`, and `LICENSE` were left alone

CI also runs `cargo audit` (ignoring `RUSTSEC-2023-0071`, `RUSTSEC-2024-0436`, and `RUSTSEC-2026-0235`) and `cargo build --release --workspace`. `RUSTFLAGS=-Dwarnings`.

## Testing

How was this tested?
