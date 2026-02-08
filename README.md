# Budget App

A web-based budget tracking application built with Rust, using the Axum web framework and SQLite database.

## Features

- **OAuth2 Authentication** - Secure login using OAuth2 (configured for Google by default)
- **Transaction Tracking** - Track both income and expenses
- **Categories** - Organize transactions with custom categories
- **Budget Summary** - View total income, expenses, and balance at a glance
- **Modern UI** - Clean interface using Pico CSS
- **No JavaScript Framework** - Simple HTML forms and plain HTTP requests

## Tech Stack

- **Backend**: Rust with Axum web framework
- **Database**: SQLite with sqlx
- **Templates**: Askama
- **Frontend**: Pico CSS with plain HTML
- **Authentication**: OAuth2

## Prerequisites

- Rust (1.70 or later)
- SQLite

## Setup

1. **Clone the repository**
   ```bash
   cd budget2
   ```

2. **Copy environment file**
   ```bash
   cp .env.example .env
   ```

3. **Configure OAuth2**
   
   For Google OAuth:
   - Go to [Google Cloud Console](https://console.cloud.google.com/apis/credentials)
   - Create a new OAuth 2.0 Client ID
   - Set authorized redirect URI to: `http://localhost:3000/auth/callback`
   - Copy the Client ID and Client Secret to your `.env` file

   Update `.env`:
   ```
   OAUTH_CLIENT_ID=your_actual_client_id
   OAUTH_CLIENT_SECRET=your_actual_client_secret
   ```

4. **Build and run**
   ```bash
   cargo build
   cargo run
   ```

   The app will be available at `http://localhost:3000`

## Database

The database is automatically created and migrated on first run. The SQLite database file will be created as `budget.db` in the project root.

## Usage

1. **Login** - Click "Login with OAuth" to authenticate
2. **Add Categories** - Create categories to organize your transactions (optional)
3. **Add Transactions** - Record income and expenses with descriptions and dates
4. **View Dashboard** - See your budget summary and recent transactions

## Project Structure

```
budget2/
├── src/
│   ├── main.rs              # Application entry point
│   ├── auth.rs              # OAuth2 authentication
│   ├── handlers/
│   │   └── mod.rs           # Route handlers
│   └── models/
│       └── mod.rs           # Database models
├── templates/
│   ├── index.html           # Landing page
│   └── dashboard.html       # Main dashboard
├── migrations/
│   └── 20240101000000_initial.sql  # Database schema
├── Cargo.toml
└── .env.example
```

## Development

Run in development mode with auto-reload:
```bash
cargo watch -x run
```

Check code:
```bash
cargo check
cargo clippy
```

## Environment Variables

- `DATABASE_URL` - SQLite database path (default: `sqlite:budget.db`)
- `OAUTH_CLIENT_ID` - OAuth2 client ID
- `OAUTH_CLIENT_SECRET` - OAuth2 client secret
- `OAUTH_AUTH_URL` - OAuth2 authorization URL
- `OAUTH_TOKEN_URL` - OAuth2 token URL
- `OAUTH_REDIRECT_URL` - OAuth2 redirect URL
- `OAUTH_USERINFO_URL` - OAuth2 user info endpoint

## License

MIT
