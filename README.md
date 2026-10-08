# Payment Engine

An asynchronous payment ledger written in Rust. It demonstrates reliable concurrent request processing with Actix Web and Tokio, durable PostgreSQL storage with SQLx, idempotent writes, structured telemetry, and production-oriented lifecycle handling.

## Quick start

Requirements: Docker with Compose v2.

```bash
docker compose up --build
```

The API is available at `http://localhost:8080`. Database migrations run automatically when the application starts.

```bash
# Create an account
CLIENT_ID=$(curl -fsS http://localhost:8080/new_client \
  -H 'content-type: application/json' \
  -d '{"client_name":"Alice","birth_date":"1990-02-15","document_number":"AR-12345678","country":"AR"}' \
  | jq -r .id)

# Credit 100.00. Repeating this exact request returns the original transaction.
curl -fsS http://localhost:8080/new_credit_transaction \
  -H 'content-type: application/json' \
  -H 'idempotency-key: credit-alice-001' \
  -d "{\"client_id\":\"$CLIENT_ID\",\"credit_amount\":\"100.00\"}"

# Debit 35.50
curl -fsS http://localhost:8080/new_debit_transaction \
  -H 'content-type: application/json' \
  -H 'idempotency-key: debit-alice-001' \
  -d "{\"client_id\":\"$CLIENT_ID\",\"debit_amount\":\"35.50\"}"

# Read the balance derived from every ledger entry
curl -fsS "http://localhost:8080/client_balance/$CLIENT_ID"

# Operational endpoints
curl -fsS http://localhost:8080/health
curl -fsS http://localhost:8080/ready
curl -fsS http://localhost:8080/metrics
```

Monetary amounts are JSON strings so clients do not lose decimal precision.

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `POST` | `/new_client` | Create an account; document numbers are unique |
| `POST` | `/new_credit_transaction` | Append a positive credit to the ledger |
| `POST` | `/new_debit_transaction` | Append a positive debit if funds are available |
| `GET` | `/client_balance/{client_id}` | Return client data and the ledger-derived balance |
| `GET` | `/health` | Process liveness; it does not depend on PostgreSQL |
| `GET` | `/ready` | Readiness, including a PostgreSQL query |
| `GET` | `/metrics` | Prometheus text metrics |

Transaction writes require an `Idempotency-Key` header containing 1–128 printable characters. Reusing a key with the same client, operation, and amount returns the original transaction with `replayed: true`. Reusing it for a different operation returns `409 Conflict`.

All errors have one shape and the response carries the same `x-correlation-id` used in the body:

```json
{
  "error": {
    "code": "insufficient_funds",
    "message": "Insufficient funds",
    "correlation_id": "81c46745-ae5c-4d76-a881-cf844a44ab53"
  }
}
```

Clients may supply `x-correlation-id`; otherwise the service creates one.

## Architecture and reliability decisions

```text
HTTP request
  -> correlation/tracing/metrics middleware
  -> validation
  -> SQL transaction
       -> advisory lock for this idempotency key
       -> row lock for this client
       -> derive current balance from ledger
       -> append immutable ledger row
  -> bounded Tokio event channel
  -> asynchronous event worker
```

- **Ledger instead of mutable state.** `clients` stores identity only. Credits and debits are append-only rows in `ledger_transactions`; balances are calculated with `SUM(credits - debits)`. A database trigger rejects updates and deletes.
- **Atomic debit decisions.** A `SELECT ... FOR UPDATE` locks one client row for the short SQL transaction containing the balance check and insert. Operations on different accounts proceed independently, and no lock is held during unrelated network or filesystem I/O.
- **Database-enforced invariants.** Foreign keys, unique document and idempotency constraints, positive-amount checks, and the transaction-kind enum complement application validation.
- **Idempotency under races.** A transaction-scoped PostgreSQL advisory lock derived from the key serializes only equal keys. The original result is then read from the ledger. The unique constraint remains a final integrity boundary.
- **Events do not weaken commits.** After commit, the handler uses non-blocking `try_send` into a bounded Tokio channel. A slow worker cannot hold a database transaction or stall a request. Dropped events are logged and counted; the committed ledger remains authoritative.
- **No global application lock.** SQLx's pool manages connections. PostgreSQL provides short, scoped synchronization where the data lives.

### Actix and Tokio execution model

`#[actix_web::main]` starts a Tokio runtime. Actix runs multiple HTTP workers, and each worker polls many request futures cooperatively. Awaiting SQLx returns the runtime thread to the executor instead of blocking it. Tokio also runs the event worker as an independent task. CPU-heavy or blocking work must not be added directly to a handler; it should use `spawn_blocking` or a separate service.

### Timeouts and shutdown

Database pool acquisition and request-header deadlines are configurable. SQL failures map to explicit JSON errors rather than panics. Actix handles `SIGINT`/`SIGTERM`, stops accepting requests, waits for in-flight work up to `SHUTDOWN_TIMEOUT_SECONDS`, closes the SQLx pool, and gives the event worker the same bounded drain period. Docker's grace period is longer than the application deadline.

## Observability

Logs are JSON and include tracing spans plus transaction/client identifiers for worker events. HTTP responses include correlation IDs. `/metrics` exposes:

- `http_request_duration_seconds`
- `http_errors_total`
- `ledger_transactions_total{kind,replayed}`
- `worker_events_total{result}`

`/health` answers as long as the process can serve HTTP. `/ready` returns `503` if PostgreSQL cannot serve a query, which makes it suitable for orchestrator readiness probes.

## Development and tests

Copy `.env.example` to `.env`, start PostgreSQL, then use stable Rust:

```bash
docker compose up -d postgres
export TEST_DATABASE_URL=postgres://payment_engine:payment_engine@localhost:5432/payment_engine
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

The integration suite covers invalid credit/debit amounts, insufficient funds, missing clients, duplicate documents, idempotent replays, concurrent operations on one account, persistence failure mapping, ledger-derived arithmetic, and ledger immutability.

CI runs formatting, Clippy, tests against PostgreSQL 17, and RustSec dependency auditing. Configuration is documented in `.env.example`.

## Limitations and next steps

- The asynchronous worker currently logs in-process events. Its bounded channel is intentionally not a durable outbox: a crash after commit can lose an event. A production extension should add an outbox table in the same SQL transaction and publish it to Kafka or Redpanda with retry tracking.
- Balances are calculated from the complete ledger on every read/write. This is correct and intentionally transparent, but a high-volume system should add snapshots/materialized aggregates while retaining the ledger as the source of truth.
- One currency/account is assumed; the schema does not yet model currency, exchange, fees, reversals, or disputes.
- Authentication, authorization, rate limiting, TLS termination, retention, and personally identifiable information controls belong at the deployment boundary and are outside this demonstration.
- Idempotency keys are globally unique and never expire. A production retention policy must match the business replay window without allowing old operations to be duplicated.

## License

MIT. See [LICENSE](LICENSE).
