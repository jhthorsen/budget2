-- Create import_rules table for automatic categorization and account assignment
CREATE TABLE IF NOT EXISTS import_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    pattern TEXT NOT NULL,
    category_id INTEGER,
    account_id INTEGER,
    priority INTEGER NOT NULL DEFAULT 999,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE SET NULL,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL
);

-- Create index for faster lookups
CREATE INDEX idx_import_rules_user_priority ON import_rules(user_id, priority);
