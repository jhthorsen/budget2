-- Add is_mine column to user_accounts table
ALTER TABLE user_accounts ADD COLUMN is_mine INTEGER NOT NULL DEFAULT 1;

-- Update existing records to be marked as "mine" by default
UPDATE user_accounts SET is_mine = 1;
