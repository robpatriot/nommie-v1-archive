# Contributing to Nommie

## Testing

Unit tests (no DB):
```bash
cargo test --manifest-path ./apps/backend/Cargo.toml --lib
```

Integration tests (DB required):
```bash
pnpm test:int
# clean slate:
RECREATE_TEST_DB=1 pnpm test:int
```

Notes:
- The test runner starts Postgres, sets `DATABASE_URL` to `${POSTGRES_DB}_test` using `APP_DB_*` creds, and runs migrations automatically.
- Tests will fail fast if `DATABASE_URL` is not a `*_test` DB.

Local DB commands:
```bash
pnpm db:start
pnpm db:logs
pnpm db:reset   # ⚠️ drops the volume and re-initializes dev + test DBs
```

Pre-commit hooks:
```bash
pnpm hooks:install
# On commit, the hook runs backend clippy, staged rustfmt, ESLint/Prettier on staged frontend files.
```
