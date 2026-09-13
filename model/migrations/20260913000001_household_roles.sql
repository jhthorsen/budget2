ALTER TABLE household_members RENAME TO household_members_old;

CREATE TABLE household_members (
    household_id INTEGER NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('manager', 'assistant', 'member')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (household_id, user_id)
);

INSERT INTO household_members (household_id, user_id, role, created_at)
SELECT household_id, user_id,
       CASE WHEN role = 'admin' THEN 'manager' ELSE role END,
       created_at
FROM household_members_old;

DROP TABLE household_members_old;

CREATE TRIGGER prevent_last_manager_delete
BEFORE DELETE ON household_members
WHEN OLD.role = 'manager'
 AND NOT EXISTS (
     SELECT 1 FROM household_members
     WHERE household_id = OLD.household_id AND role = 'manager' AND user_id <> OLD.user_id
 )
BEGIN
    SELECT RAISE(ABORT, 'household must have at least one manager');
END;

CREATE TRIGGER prevent_last_manager_demotion
BEFORE UPDATE OF role ON household_members
WHEN OLD.role = 'manager' AND NEW.role <> 'manager'
 AND NOT EXISTS (
     SELECT 1 FROM household_members
     WHERE household_id = OLD.household_id AND role = 'manager' AND user_id <> OLD.user_id
 )
BEGIN
    SELECT RAISE(ABORT, 'household must have at least one manager');
END;
