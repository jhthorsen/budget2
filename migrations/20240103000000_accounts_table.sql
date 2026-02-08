-- Create accounts table
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Create junction table for user-account relationship (many-to-many)
CREATE TABLE IF NOT EXISTS user_accounts (
    user_id INTEGER NOT NULL,
    account_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, account_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE INDEX idx_user_accounts_user ON user_accounts(user_id);
CREATE INDEX idx_user_accounts_account ON user_accounts(account_id);

-- Migrate existing account data
-- First, insert unique account names into accounts table
INSERT INTO accounts (name)
SELECT DISTINCT account
FROM transactions
WHERE account IS NOT NULL AND account != ''
ORDER BY account;

-- Create a temporary column for the new foreign key
ALTER TABLE transactions ADD COLUMN account_id INTEGER;

-- Populate account_id based on account name
UPDATE transactions
SET account_id = (
    SELECT id FROM accounts WHERE accounts.name = transactions.account
)
WHERE account IS NOT NULL AND account != '';

-- Create user_accounts entries for all users who have transactions with these accounts
INSERT INTO user_accounts (user_id, account_id)
SELECT DISTINCT t.user_id, t.account_id
FROM transactions t
WHERE t.account_id IS NOT NULL;

-- Drop the old account column
-- Note: SQLite doesn't support DROP COLUMN directly in older versions
-- We'll leave it for now and use account_id going forward
-- Or recreate the table if needed

CREATE INDEX idx_transactions_account_id ON transactions(account_id);
