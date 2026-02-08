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

## Database

The database is automatically created and migrated on first run. The SQLite database file will be created as `budget.db` in the project root.

## Usage

1. **Login** - Click "Login with OAuth" to authenticate
2. **Add Accounts** - Create accounts to organize your transactions (e.g., Checking, Savings, Credit Card)
   - Accounts are shared - multiple users can access the same account
   - Mark accounts as "Mine" or "Not Mine" in the "Manage Accounts" section
   - Your balance only includes transactions from accounts marked as "Mine"
   - Great for household budgets where some accounts are shared but not owned by everyone
3. **Add Categories** - Create categories to organize your transactions (categories are user-specific)
4. **Add Transactions** - Record income and expenses with descriptions, dates, accounts, and categories
5. **Import CSV** - Bulk import transactions from CSV files:
   - Click "Import CSV" button on the dashboard
   - Upload your CSV file
   - Map CSV columns to transaction fields
   - Accounts and categories will be created automatically if they don't exist
   - Newly created accounts are marked as "Mine" by default
   - Review import results showing any errors
6. **Filter & Search** - Use the filter form to find specific transactions
7. **View Dashboard** - See your budget summary (only "Mine" accounts) and recent transactions

## Project Structure

```
budget2/
├── src/
│   ├── main.rs              # Application entry point
│   ├── auth.rs              # OAuth2 authentication
│   ├── handlers/
│   │   ├── mod.rs           # Route handlers
│   │   └── csv.rs           # CSV import handlers
│   └── models/
│       └── mod.rs           # Database models
├── templates/
│   ├── index.html           # Landing page
│   ├── dashboard.html       # Main dashboard
│   ├── csv_upload.html      # CSV upload page
│   ├── csv_mapping.html     # Column mapping page
│   └── csv_result.html      # Import results page
├── migrations/
│   ├── 20240101000000_initial.sql       # Database schema
│   ├── 20240102000000_add_account.sql   # Account field migration
│   ├── 20240103000000_accounts_table.sql # Accounts table with sharing
│   └── 20240104000000_add_is_mine.sql   # Account ownership marking
├── Cargo.toml
└── .env.example
```

## Database Schema

### Key Tables
- **users** - User accounts from OAuth
- **accounts** - Shared accounts (Checking, Savings, etc.)
- **user_accounts** - Many-to-many relationship for account sharing
- **categories** - User-specific transaction categories
- **transactions** - Financial transactions linked to users, accounts, and categories

### Account Sharing
Accounts are designed to be shared between users:
- When an account is created, it's added to the `accounts` table
- The creator is automatically linked via `user_accounts` table with `is_mine = 1`
- Other users can be given access to the same account
- Each user can mark accounts as "Mine" (included in balance) or "Not Mine" (excluded from balance)
- Perfect for scenarios like:
  - **Couples**: Shared checking account, but each person has their own savings
  - **Roommates**: Shared utilities account, but personal credit cards
  - **Families**: Parents can see children's accounts but exclude them from their own balance
  - **Business**: Shared company account, but personal expense accounts

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
- `OAUTH_REDIRECT_URL` - OAuth2 redirect URL (e.g., `http://localhost:3000/auth/callback`)
- `OAUTH_DISCOVERY_URL` - (Optional) OIDC discovery URL. Defaults to Google's discovery endpoint.
  - Google: `https://accounts.google.com/.well-known/openid-configuration` (default)
  - Microsoft: `https://login.microsoftonline.com/common/v2.0/.well-known/openid-configuration`
  - GitHub: `https://token.actions.githubusercontent.com/.well-known/openid-configuration`

The app automatically fetches authorization, token, and userinfo endpoints from the discovery URL, so you don't need to configure them separately.

## CSV Import Format

The CSV import feature supports flexible column mapping. Your CSV file should:

- Have a header row with column names
- Include the following data:
  - **Date**: YYYY-MM-DD or YYYY/MM/DD format
  - **Amount**: Numeric value ($ and commas are automatically removed)
  - **Description**: Transaction description
  - **Type**: Either map to a CSV column containing "income" or "expense", OR set a fixed value for all rows
  - **Account** (optional): Either map to a CSV column, OR set a fixed value for all rows (e.g., "Checking")
  - **Category** (optional): Category names will be automatically created if they don't exist

### Automatic Category Creation

When importing transactions with categories, the system will:
- Check if the category already exists for your user
- If it exists, use the existing category
- If it doesn't exist, automatically create a new category with that name
- Show you a list of all newly created categories after import

This means you don't need to pre-create all categories before importing!

### Fixed Values

You can use fixed values instead of CSV columns for:
- **Type**: If all transactions in the CSV are the same type (e.g., all expenses)
- **Account**: If all transactions are from the same account (e.g., all from "Checking")

This is useful when importing bank statements that don't include these fields.

Example CSV with type column:
```csv
Date,Description,Amount,Type,Account,Category
2024-01-15,Salary,$3000.00,income,Checking,
2024-01-16,Groceries,125.50,expense,Checking,Food
2024/01/17,Electric Bill,85.00,expense,Checking,Utilities
```

Example CSV without type column (using fixed value):
```csv
Date,Description,Amount,Category
2024-01-16,Groceries,125.50,Food
2024/01/17,Electric Bill,85.00,Utilities
2024/01/18,Internet,60.00,Utilities
```
*In this case, you would set Type to "expense" and Account to "Checking" as fixed values during import.*

The import process will:
1. Show you all CSV column headers
2. Let you map them to transaction fields
3. Import all valid rows
4. Report any rows that failed with detailed error messages
5. Automatically delete the temporary CSV file after processing

## License

MIT
