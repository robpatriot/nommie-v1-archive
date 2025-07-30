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

### 🧪 Run tests (TODO)

```bash
# Placeholder
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
