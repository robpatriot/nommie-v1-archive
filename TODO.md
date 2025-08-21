# TODO list (Nommie backend)

## Milestone A: Integration testing & DB bootstrap
- Ensure tests use only the configurator + inline App building pattern (no App<T> returns).
- Finalize integration test harness: `tests/smoke.rs` uses Actix in-process, guard for *_test DB, and coarse assertions for create→add AI→bids→pick trump→snapshot.
- Centralize DB bootstrap: `connect_and_migrate_from_env()` helper + OnceCell to run SeaORM `Migrator::up` once for all tests.
- Put the helper in `integration_tests.rs` and export a `shared_db()` that other tests can reuse.
- **Acceptance:** smoke test runs green with `pnpm run test:int`.

## Milestone B: Init script & migrations
- Keep `init.sh` infra-only (role, DBs, schema, grants, search_path). No indexes or tables.
- ✅ Move all index DDL into a SeaORM migration.
- Verify migrations create full schema + indexes for both `nommie` and `nommie_test` when run programmatically.
- **Acceptance:** dropping/recreating DB via `pnpm db:reset` yields a working schema + indexes.

## Milestone C: Actix app wiring
- Use ONLY a route configurator: `pub fn configure_routes(cfg: &mut ServiceConfig)`.
- Build `App` inline in `main.rs` and in tests with `.configure(configure_routes)` and `.app_data(Data::new(db.clone()))`.
- Avoid factories/builders that return `App`.
- **Acceptance:** build compiles without generic `App<T>` issues.

## Milestone D: Phase-4 safety guards
- Enforce *_test DB in tests (existing guard).
- Add a panic guard in test bootstrap if `DATABASE_URL` lacks *_test.
- Optional: runtime warning (not panic) in `main.rs` when `APP_ENV=test` but DB isn’t *_test.
- **Acceptance:** tests panic if run against a non-*_test DB.

## Milestone E: CI & scripts
- Add CI job: start Postgres service, wait-for-pg, export `DATABASE_URL=…/_test`, run `cargo test`.
- Add local script/Make target (e.g., `pnpm test:int`): `docker compose up -d postgres` → wait-for-pg → set `DATABASE_URL` to *_test → `cargo test`.
- **Acceptance:** CI green for tests + clippy.

## Milestone F: Documentation
- Update README/CONTRIBUTING: how to run DB, migrations, and tests; explain *_test guard; explain configurator pattern.
- Document the “reset flow” (down -v → up -d → migrations run on first test/start).
- **Acceptance:** devs can follow docs to reset + test DB easily.

## Milestone G: Refactor plan (ACTIVE)
- ✅ Split `game_management.rs` into modules: `rules.rs`, `scoring.rs`, `bidding.rs`, `tricks.rs`, `state.rs`.
- ✅ Move unit tests alongside new modules; keep integration tests under `tests/`.
- Clean up `mod.rs`:
  - Remove `#[allow(dead_code)]` and unused imports.
  - Tighten visibility to `pub(crate)` where possible; no re-exports.
  - Keep only wiring + thin orchestration + truly cross-cutting items.
  - Add concise module-level doc comment mapping submodules.
  - Add `// TODO(next milestone):` stubs for round advancement + AI orchestration.
- **Verification:** `pnpm run backend:clippy` and `pnpm run test:int` clean.
- **Acceptance:** code compiles, tests green, minimal API at `mod.rs`.  
  Round advancement + AI orchestration are **explicitly out of scope** (moved to Milestone K).

## Milestone H: Async lock cleanup (deferred until after refactor)
- Replace inappropriate `tokio::sync::Mutex` in sync contexts; prefer `std::sync::Mutex` or refactor to async.
- Audit locking strategy in game state + snapshot code; ensure no `.await` in sync handlers.
- **Acceptance:** TBD when milestone resumes.

## Milestone I: Known issues / deferred tasks
- ⚠️ Investigate `pnpm run test:int` error → capture logs, classify infra vs. logic.
- ⚠️ Verify test harness teardown/DB cleanup is reliable (avoid DB state leakage across tests).
- ⚠️ Add coverage for error paths (invalid bids, invalid trick plays).
- ⚠️ Combine schema creation logic (currently duplicated in `docker/postgres/init.sh` and `scripts/test-int.sh`) into a shared script so both use the same code path.

## Milestone J: Testing improvements
- Deterministic RNG + test utilities (backend) — seeded RNG; helpers for reproducibility.
- Backend test expansion — focused integrations, units, small proptests.
- Frontend testing scaffold — Vitest + RTL + MSW; 2–3 unit/integration; 1 minimal Playwright e2e.
- DB test isolation pattern — txn-per-test first, schema-per-run fallback; **acceptance:** parallel tests safe.
- Coverage signal (lightweight) — backend llvm-cov vs tarpaulin (to discuss); frontend Vitest coverage.
- Mutation testing pilot — `cargo-mutants` on tricks + scoring.
- Performance sanity benches — Criterion benches for trick winner + scoring.

## Milestone K: Round advancement + AI orchestration
- Reintroduce round progression logic after refactor.
- Reintroduce AI player orchestration (bidding + trick play).
- **Acceptance:** integration test covers full game loop (deal → bid → trump → tricks → scoring → round advance).

## Milestone L: Auth & request validation via Actix extractors
- **Investigate viability:** Can custom extractors (e.g., for JWT → `AuthedUser`, membership → `GameContext`) centralize checks like “is logged in,” “has valid token,” “is in this game,” etc.?
- **Design extractors:** 
  - `AuthExtractor` → validates JWT, loads user, attaches `UserId`.
  - `GameExtractor` → validates game ID, ensures user is a participant, attaches `GameId` + role.
  - Consistent error mapping to HTTP statuses (401/403/404) and error body shape.
- **Refactor handlers:** Replace ad-hoc checks across `game_management` submodules with typed inputs from extractors; keep business logic pure.
- **Testing:** Unit tests for extractors (happy + failure paths), integration tests for endpoints using them.
- **Acceptance:** Handlers no longer duplicate auth/membership checks; responsibilities move to extractors; tests and clippy green.
