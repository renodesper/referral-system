# Referral System

This is a referral system that allows users to earn rewards for referring their friends.

## Prerequisites

Make sure to have PostgreSQL installed and running. The DDL and seed data are provided below.

### Users

```sql
CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY,
    referrer_id BIGINT REFERENCES users (id),
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);
```

### Balances

```sql
CREATE TABLE IF NOT EXISTS balances (
    user_id BIGINT PRIMARY KEY REFERENCES users (id),
    balance BIGINT NOT NULL DEFAULT 0
);
```

### Purchases

```sql
CREATE TABLE IF NOT EXISTS purchases (
    id UUID PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id),
    amount BIGINT NOT NULL CHECK (amount >= 0),
    status TEXT NOT NULL CHECK (
        status IN ('authorized', 'captured', 'refunded', 'voided')
    )
);
```

### Rewards

```sql
CREATE TABLE IF NOT EXISTS rewards (
    id BIGSERIAL PRIMARY KEY,
    purchase_id UUID NOT NULL REFERENCES purchases (id),
    user_id BIGINT NOT NULL REFERENCES users (id),
    beneficiary_user_id BIGINT NOT NULL REFERENCES users (id),
    level INT NOT NULL CHECK (level IN (1, 2)),
    amount BIGINT NOT NULL CHECK (amount >= 0),
    UNIQUE (purchase_id, beneficiary_user_id, level)
);
```

### Indexes

```sql
CREATE INDEX IF NOT EXISTS purchases_user_idx ON purchases (user_id);
CREATE INDEX IF NOT EXISTS rewards_user_idx ON rewards (user_id);
CREATE INDEX IF NOT EXISTS rewards_beneficiary_idx ON rewards (beneficiary_user_id);
```

### Seed Data

```sql
INSERT INTO users (id, referrer_id, is_active)
VALUES (1, NULL, TRUE) ON CONFLICT DO NOTHING;

INSERT INTO users (id, referrer_id, is_active)
VALUES (2, 1, TRUE) ON CONFLICT DO NOTHING;

INSERT INTO users (id, referrer_id, is_active)
VALUES (3, 2, TRUE) ON CONFLICT DO NOTHING;

INSERT INTO users (id, referrer_id, is_active)
VALUES (4, NULL, FALSE) ON CONFLICT DO NOTHING;

INSERT INTO users (id, referrer_id, is_active)
VALUES (5, 4, TRUE) ON CONFLICT DO NOTHING;

INSERT INTO purchases (id, user_id, amount, status)
VALUES
    (
        '11111111-1111-1111-1111-111111111111',
        3,
        10000,
        'captured'
    ) ON CONFLICT DO NOTHING;

INSERT INTO purchases (id, user_id, amount, status)
VALUES
    (
        '22222222-2222-2222-2222-222222222222',
        5,
        10000,
        'captured'
    ) ON CONFLICT DO NOTHING;
```

## How to Test

There are 4 endpoints (3 if we exclude the health check):

1. `GET /health`: To check the health of the system.

```sh
curl -X GET http://localhost:8080/health
```

2. `GET /balances/{user_id}`: To retrieve user's balances.

```sh
curl -X GET 'http://localhost:8000/balances/1'
curl -X GET 'http://localhost:8000/balances/2'
curl -X GET 'http://localhost:8000/balances/3'
curl -X GET 'http://localhost:8000/balances/4'
curl -X GET 'http://localhost:8000/balances/5'
```

3. `POST /purchases`: To create a new purchase.

```sh
curl -X POST 'http://localhost:8000/purchases' \
--header 'Content-Type: application/json' \
--data '{
    "user_id": 5,
    "amount": 10000,
    "status": "captured"
}'
```

4. `POST /process/{purchase_id}`: To process a referral based on `purchase_id`.

```sh
curl -X POST 'http://localhost:8000/process/11111111-1111-1111-1111-111111111111'
curl -X POST 'http://localhost:8000/process/22222222-2222-2222-2222-222222222222'
```

> P.S. Processing a referral has its own endpoint since I think it is better to call it using a queue.
