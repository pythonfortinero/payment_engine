CREATE TYPE transaction_kind AS ENUM ('credit', 'debit');

CREATE TABLE clients (
    id UUID PRIMARY KEY,
    client_name TEXT NOT NULL CHECK (length(trim(client_name)) > 0),
    birth_date DATE NOT NULL,
    document_number TEXT NOT NULL UNIQUE CHECK (length(trim(document_number)) > 0),
    country VARCHAR(2) NOT NULL CHECK (country ~ '^[A-Z]{2}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE ledger_transactions (
    id UUID PRIMARY KEY,
    client_id UUID NOT NULL REFERENCES clients(id),
    kind transaction_kind NOT NULL,
    amount NUMERIC(20, 4) NOT NULL CHECK (amount > 0),
    idempotency_key VARCHAR(128) NOT NULL UNIQUE CHECK (length(trim(idempotency_key)) > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ledger_transactions_client_created_idx
    ON ledger_transactions (client_id, created_at, id);
