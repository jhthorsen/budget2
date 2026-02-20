# Budget App

A web-based budget tracking application built with Rust, using the Axum web framework and SQLite database.

## Features

- **OAuth2 Authentication** - Secure login using OAuth2 (configured for Google by default)
- **Transaction Tracking** - Track both income and expenses
- **Shared Accounts** - Create accounts (Checking, Savings, Credit Card, etc.) that can be shared between multiple users
  - Mark accounts as "Mine" or "Not Mine" to control balance calculations
  - Perfect for household budgets where some accounts are shared but not owned by everyone
- **Categories** - Organize transactions with custom categories (user-specific)
- **CSV Import** - Import transactions from CSV files with flexible column mapping
  - Automatically creates missing categories and accounts
- **Advanced Filtering** - Filter transactions by:
  - Search text (description)
  - Transaction type (income/expense)
  - Category
  - Account
  - Date range (from/to)
- **Pagination** - Navigate through transactions with customizable page size (10, 20, 50, or 100 per page)
- **Smart Budget Summary** - View total income, expenses, and balance
  - Only includes transactions from accounts marked as "Mine"
  - Allows accurate personal balance even when sharing accounts
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
   
   The app uses OpenID Connect Discovery for automatic OAuth configuration.
   
   For Google OAuth:
   - Go to [Google Cloud Console](https://console.cloud.google.com/apis/credentials)
   - Create a new OAuth 2.0 Client ID
   - Set authorized redirect URI to: `http://localhost:3000/auth/callback`
   - Copy the Client ID and Client Secret to your `.env` file

   Update `.env`:
   ```
   OAUTH_CLIENT_ID=your_actual_client_id
   OAUTH_CLIENT_SECRET=your_actual_client_secret
   OAUTH_REDIRECT_URL=http://localhost:3000/auth/callback
   ```

   For other providers (Microsoft, GitHub, etc.), add the discovery URL:
   ```
   OAUTH_DISCOVERY_URL=https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration
   ```

4. **Build and run**
   ```bash
   cargo build
   cargo run
   ```

   The app will be available at `http://localhost:3000`

## Environment Variables

- `DATABASE_URL` - SQLite database path (default: `sqlite:budget.db`)
- `OAUTH_CLIENT_ID` - OAuth2 client ID
- `OAUTH_CLIENT_SECRET` - OAuth2 client secret
- `OAUTH_REDIRECT_URL` - OAuth2 redirect URL (e.g., `http://localhost:3000/auth/callback`)
- `OAUTH_DISCOVERY_URL` - (Optional) OIDC discovery URL. Defaults to Google's discovery endpoint.
  - Google: `https://accounts.google.com/.well-known/openid-configuration` (default)
  - Microsoft: `https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration`
  - GitHub: `https://token.actions.githubusercontent.com/.well-known/openid-configuration`

The app automatically fetches authorization, token, and userinfo endpoints from the discovery URL, so you don't need to configure them separately.

## Database

The database is automatically created and migrated on first run. The SQLite database file will be created as `budget.db` in the project root.

## Database Schema

```
users.id --- accounts.user_id
         |   accounts.id --- transactions.account_id
         |
         '-- transactions.user_id
             transactions.account_id --- accounts.id
             transactions.category_id ~~~ categories.id

import_rules.account_id ~~ accounts.id
import_rules.category_id ~~ categories.id
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

## License

MIT
