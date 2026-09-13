CREATE TABLE households (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE household_members (
    household_id INTEGER NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('admin', 'member')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (household_id, user_id)
);

INSERT INTO households (name) VALUES ('Family');

ALTER TABLE accounts ADD COLUMN household_id INTEGER NOT NULL DEFAULT 1 REFERENCES households(id);
ALTER TABLE categories ADD COLUMN household_id INTEGER NOT NULL DEFAULT 1 REFERENCES households(id);
ALTER TABLE import_rules ADD COLUMN household_id INTEGER NOT NULL DEFAULT 1 REFERENCES households(id);
ALTER TABLE transactions ADD COLUMN imported_by_user_id INTEGER REFERENCES users(id);

UPDATE transactions SET imported_by_user_id = user_id WHERE imported_by_user_id IS NULL;

CREATE INDEX idx_accounts_household ON accounts(household_id);
DROP INDEX IF EXISTS idx_accounts_name;
CREATE UNIQUE INDEX idx_accounts_household_name ON accounts(household_id, name);
CREATE INDEX idx_categories_household ON categories(household_id);
CREATE INDEX idx_import_rules_household ON import_rules(household_id);
CREATE INDEX idx_transactions_imported_by ON transactions(imported_by_user_id);

CREATE TRIGGER prevent_last_admin_delete
BEFORE DELETE ON household_members
WHEN OLD.role = 'admin'
 AND NOT EXISTS (
     SELECT 1 FROM household_members
     WHERE household_id = OLD.household_id AND role = 'admin' AND user_id <> OLD.user_id
 )
BEGIN
    SELECT RAISE(ABORT, 'household must have at least one admin');
END;

CREATE TRIGGER prevent_last_admin_demotion
BEFORE UPDATE OF role ON household_members
WHEN OLD.role = 'admin' AND NEW.role <> 'admin'
 AND NOT EXISTS (
     SELECT 1 FROM household_members
     WHERE household_id = OLD.household_id AND role = 'admin' AND user_id <> OLD.user_id
 )
BEGIN
    SELECT RAISE(ABORT, 'household must have at least one admin');
END;
