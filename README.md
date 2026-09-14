# Budget2

A personal-finance web app for tracking accounts, categories, transactions, and budgets across a household.

## Features

- Dashboard with transaction filters and spending summaries
- Household accounts with manager, assistant, and member roles
- CSV transaction imports with column mapping and reusable import rules
- OpenID Connect sign-in
- SQLite storage and automatic schema migrations

## Run Locally

For a quick local session, use the built-in offline authentication mode:

```sh
export OFFLINE_OIDC_ID=local-user
export OFFLINE_OIDC_EMAIL=you@example.com
export OFFLINE_OIDC_NAME='Local User'
export SECURE_SESSION=false
cargo run --package budget2-web --features offline
```

Open <http://127.0.0.1:3000>. The application creates its SQLite database at `local/budget2.db` by default.

To use a different database location or listen address, set `DATABASE_URL`, `HOST`, and `PORT` before starting the server.

## OpenID Connect

The normal build requires these environment variables:

```sh
export OIDC_CLIENT_ID=...
export OIDC_CLIENT_SECRET=...
export OIDC_DISCOVERY_URL=https://issuer.example/.well-known/openid-configuration
export OIDC_REDIRECT_URL=https://budget.example/auth/callback
cargo run --package budget2-web
```

Set `SECURE_SESSION=true` (the default) when the app is served over HTTPS.

## Docker

```sh
docker run --rm -p 3000:3000 \
  -e OIDC_CLIENT_ID \
  -e OIDC_CLIENT_SECRET \
  -e OIDC_DISCOVERY_URL \
  -e OIDC_REDIRECT_URL \
  ghcr.io/jhthorsen/budget2:latest
```

Mount `/app/local` if the SQLite database must persist across container replacements.

## Development

```sh
cargo test --workspace
cargo fmt --check
```

## License

MIT
