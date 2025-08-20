# 🃏 Nommie

**Nommie** is a full-stack, web-based, multiplayer implementation of **Nomination Whist**, designed for 4 players. The project focuses on fair bidding, strategic play, and seamless multiplayer interaction — all backed by a modern, scalable architecture.

---

## 🚀 Tech Stack

### 🖥️ Frontend
- [Next.js](https://nextjs.org/)
- [Tailwind CSS](https://tailwindcss.com/)

### 🔐 Authentication
- [NextAuth.js](https://next-auth.js.org/) (Google OAuth)
- JWTs signed with `HS256` using [`jose`](https://github.com/panva/jose)

### ⚙️ Backend
- [Rust](https://www.rust-lang.org/) + [Actix Web](https://actix.rs/)
- [SeaORM](https://www.sea-ql.org/SeaORM/)

### 🗄️ Database
- PostgreSQL (via Docker Compose)

### 🧰 Tooling
- pnpm
- Docker Desktop
- WSL2

---

## 🎮 Game Rules

- **Exactly 4 players per game**
- Players **bid publicly** each round
- **Highest bidder chooses trump**
  - Ties resolved by turn order
- **Scoring:**
  - +1 point per trick
  - +10 bonus if tricks = bid

## 🧾 Round Structure

Total: **26 rounds**

- Start with **13 cards/player**
- Decrease by 1 card per round down to 2
- Play **4 rounds at 2 cards**
- Increase back up to 13

---

## 🛠️ Development

### ▶️ Start the app

```bash
pnpm dev:full
```

Starts:
- Rust backend (hot-reloading via `cargo watch`)
- Next.js frontend (Turbopack)
- Dockerized PostgreSQL

### 🐳 Docker

To start the database:

```bash
docker compose up -d db
```

To tear it down:

```bash
docker compose down
```

## Local DB & Tests

### Environment
Copy `.env.example` → `.env` and adjust values if needed.  
- `POSTGRES_USER` / `POSTGRES_PASSWORD`: superuser that Docker entrypoint uses.  
- `APP_DB_USER` / `APP_DB_PASSWORD`: application role (created automatically).  
- `POSTGRES_DB`: base name (defaults to `nommie`). Tests will use `${POSTGRES_DB}_test`.

### Start / Stop / Reset Postgres
pnpm db:start     # start postgres container
pnpm db:stop      # stop postgres container
pnpm db:logs      # view logs during init
pnpm db:reset     # ⚠ drop volume and re-init (dev + test DBs)

## Running Tests (Unit + Integration)

Run both sets of tests with one command:

```bash
pnpm test:int
```

What it does:
- Starts Postgres if needed (same as `pnpm db:start`)
- Waits until the DB is ready
- Exports a safe `DATABASE_URL` that targets `${POSTGRES_DB}_test` and uses the `APP_DB_*` credentials from `.env`
- Runs backend unit tests (pure, no DB) then integration tests (migrations run automatically via the test bootstrap)

Fresh, clean test DB (drop and recreate only the `*_test` database):
```bash
RECREATE_TEST_DB=1 pnpm test:int
```

Safety guard:
- Integration tests refuse to run unless `DATABASE_URL` contains `"_test"`.
- If you see "Refusing to run unless DATABASE_URL points to a *_test database.", fix your env or let `pnpm test:int` set it for you.

Common pitfalls:
- "password authentication failed for user 'nommie'" → You connected as `POSTGRES_USER` instead of `APP_DB_USER`. Ensure `.env` has `APP_DB_USER` / `APP_DB_PASSWORD` and re-run `pnpm test:int`.
- Container not ready → `pnpm db:logs` to watch `init.sh`.

Lint/format helpers:
```bash
pnpm backend:clippy
pnpm backend:fmt
pnpm frontend:lint -- .
pnpm frontend:fmt -- .
```

---

## 🔐 Authentication

- Google sign-in via NextAuth.js
- JWTs signed with `HS256` using shared `AUTH_SECRET`
- Tokens are verified by the Rust backend and carry `sub`, `email`, `iat`, and `exp` claims

---

## 📦 Project Structure

```text
apps/
  frontend/      # Next.js frontend
  backend/       # Actix Web backend
docker/
  postgres/      # Database config
```

---

## 📅 Roadmap

- [x] End-to-end Google login with JWT
- [x] Secure protected backend routes
- [x] Fresh token generation and caching
- [ ] Create users in DB on login
- [ ] Game creation & lobby management
- [ ] Core game engine (rounds, bidding, scoring)
- [ ] WebSocket multiplayer (planned)

---

## 🪪 License

[MIT License](LICENSE)

---

## 👨‍💻 Author

Rob Denison  
Built using [Cursor](https://cursor.sh/), ChatGPT, and way too many coffee breaks ☕
