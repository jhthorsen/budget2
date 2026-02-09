-- Add original_amount column to store pre-multiplier values
ALTER TABLE transactions ADD COLUMN original_amount REAL;

-- Set original_amount to current amount for existing records
UPDATE transactions SET original_amount = amount WHERE original_amount IS NULL;
