# Railway Deployment

This deployment runs Strawtea as one Railway web service plus one Railway PostgreSQL service.

The Docker image builds:

- `strawtea-ui` into static files
- `strawtea-be` into a release binary
- a runtime `/config.js` file from Railway variables

The backend serves:

- `/api/*` from Axum
- `/healthz` for Railway health checks
- `/` and frontend routes from the Svelte build

## Railway Project

1. Create a Railway project.
2. Add a PostgreSQL database service.
3. Add the GitHub repo as a service.
4. Railway should detect `railway.toml` and build with the root `Dockerfile`.

## Service Variables

Set these on the web service:

```text
DATABASE_URL=${{ Postgres.DATABASE_URL }}
SUPABASE_JWT_ISSUER=https://your-project.supabase.co/auth/v1
SUPABASE_JWT_AUDIENCE=authenticated
SUPABASE_JWT_JWKS_URL=https://your-project.supabase.co/auth/v1/.well-known/jwks.json
STRAWTEA_TWELVE_API_KEY=replace-me
VITE_SUPABASE_URL=https://your-project.supabase.co
VITE_SUPABASE_ANON_KEY=replace-me
VITE_API_BASE_URL=
```

`VITE_API_BASE_URL` should stay empty for same-origin `/api` requests.

Railway provides `PORT`; the backend uses it automatically when `HTTP_ADDR` is not set.

## Supabase

Add the Railway production domain to Supabase auth redirect URLs.

For example:

```text
https://strawtea.up.railway.app
```

If you add a custom domain, add that too.

## Database Migrations

The backend runs SQLx migrations on startup. No separate migration command is required.

## Health Check

Railway uses:

```text
/healthz
```

Configured in `railway.toml`.
