# Strawtea

Local-first setup for the Strawtea Rust backend and Svelte PWA.

## Local Database

Start PostgreSQL:

```bash
docker compose up -d db
```

The backend uses:

```text
DATABASE_URL=postgres://postgres:postgres@localhost:5432/strawtea
```

Migrations run automatically when the backend starts.

## Backend

```bash
cd strawtea-be
cp .env.example .env
direnv allow
cargo run
```

The backend loads non-secret values from `strawtea-be/.env`. Keep the Twelve Data key in your shell:

```bash
export STRAWTEA_TWELVE_API_KEY=...
```

`TWELVE_DATA_API_KEY` is also accepted as a fallback.

Health check:

```bash
curl http://127.0.0.1:8080/healthz
```

## Frontend

```bash
cd strawtea-ui
cp .env.example .env
direnv allow
npm install
npm run dev
```

Open the Vite URL, usually `http://127.0.0.1:5173`. For LAN or mapped-port testing, use the URL Vite or your port mapper exposes. When `VITE_API_BASE_URL` is empty, frontend requests use same-origin `/api` and Vite proxies them to the backend at `http://127.0.0.1:8080`.

## Checks

```bash
cd strawtea-be
cargo test

cd ../strawtea-ui
npm run check
npm run build
```
