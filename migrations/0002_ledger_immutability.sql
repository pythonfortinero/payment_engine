CREATE FUNCTION reject_ledger_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'ledger transactions are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ledger_transactions_no_update_or_delete
    BEFORE UPDATE OR DELETE ON ledger_transactions
    FOR EACH ROW EXECUTE FUNCTION reject_ledger_mutation();
