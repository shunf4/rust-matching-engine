-- Your SQL goes here
CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    password_hashed VARCHAR(122) NOT NULL,
    name VARCHAR NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL,
    balance BIGINT NOT NULL
);

CREATE TABLE stocks (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR NOT NULL UNIQUE,
    into_market BOOLEAN NOT NULL DEFAULT TRUE,
    into_market_at TIMESTAMP
);

CREATE TABLE new_stocks (
    id BIGSERIAL PRIMARY KEY REFERENCES stocks(id),
    issuer_id BIGSERIAL REFERENCES users(id),
    offer_circ BIGINT NOT NULL,
    offer_price INTEGER NOT NULL,
    offer_unfulfilled BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE user_stock (
    user_id BIGSERIAL REFERENCES users(id),
    stock_id BIGSERIAL REFERENCES stocks(id),
    hold BIGINT NOT NULL,
    PRIMARY KEY (user_id, stock_id)
);

CREATE TABLE user_ask_orders ( -- 买入委托
    id BIGSERIAL PRIMARY KEY,
    user_id BIGSERIAL REFERENCES users(id),
    stock_id BIGSERIAL REFERENCES stocks(id),
    price INTEGER NOT NULL,
    volume BIGINT NOT NULL,
    unfulfilled BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL
);
CREATE INDEX ask_orders_index ON user_ask_orders(stock_id, price, created_at, unfulfilled);

CREATE TABLE user_bid_orders ( -- 卖出委托
    id BIGSERIAL PRIMARY KEY,
    user_id BIGSERIAL REFERENCES users(id),
    stock_id BIGSERIAL REFERENCES stocks(id),
    price INTEGER NOT NULL,
    volume BIGINT NOT NULL,
    unfulfilled BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL
);
CREATE INDEX bid_orders_index ON user_bid_orders(stock_id, price, created_at, unfulfilled);

CREATE TABLE deals (
    id BIGSERIAL PRIMARY KEY,
    buy_user_id BIGSERIAL REFERENCES users(id),
    sell_user_id BIGINT REFERENCES users(id) NULL,  --- 当是 NULL 时，表示是购买发行新股
    stock_id BIGSERIAL REFERENCES stocks(id),
    price INTEGER NOT NULL,
    amount BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL    -- 成交时间，不是委托时间
);
CREATE INDEX deals_stock_id_and_price ON user_bid_orders(stock_id, price);
