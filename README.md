# Payment Engine

Este proyecto es un procesador de pagos simple hecho en Rust. No persiste nada en base de datos, todo el estado se mantiene en memoria. Sirve para testear flujos de creación de clientes, acreditación, débito, consulta y "cierre" de balances.

## Stack

- **Rust 1.88**
- Actix-web 4
- Tokio 1
- Serde
- rust_decimal
- UUID
- Chrono

## Endpoints

| Método | Ruta                          | Descripción                          |
| ------ | ----------------------------- | ------------------------------------ |
| POST   | `/new_client`                 | Crea cliente. Body: `{client_name, birth_date, document_number, country}` |
| POST   | `/new_credit_transaction`     | Acredita saldo. Body: `{client_id, credit_amount}` |
| POST   | `/new_debit_transaction`      | Debita saldo. Body: `{client_id, debit_amount}`   |
| GET    | `/client_balance/{client_id}` | Devuelve info y balance del cliente  |
| POST   | `/store_balances`             | Persiste balances en archivo y resetea |

## Correr en local

```bash
# compilar y correr
cargo run

# correr tests
cargo test
```

El servidor levanta en `http://127.0.0.1:8080`.

### Ejemplo rápido con `curl`

```bash
# Crear cliente
CID=$(curl -sX POST http://localhost:8080/new_client \
  -H "Content-Type: application/json" \
  -d '{
        "client_name":"Alice",
        "birth_date":"1990-02-15",
        "document_number":"12345678",
        "country":"AR"
      }' | jq -r '.id')

# Acreditar 1000
curl -X POST localhost:8080/new_credit_transaction \
  -H "Content-Type: application/json" \
  -d "{\"client_id\":\"$CID\",\"credit_amount\":\"1000\"}"

# Debitar 130
curl -X POST localhost:8080/new_debit_transaction \
  -H "Content-Type: application/json" \
  -d "{\"client_id\":\"$CID\",\"debit_amount\":\"130\"}"

# Consultar saldo
curl localhost:8080/client_balance/$CID

# Persistir balances en archivo
curl -X POST http://localhost:8080/store_balances
```
