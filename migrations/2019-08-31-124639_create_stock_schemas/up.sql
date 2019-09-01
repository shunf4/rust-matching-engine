-- Your SQL goes here
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    password_hashed VARCHAR(122) NOT NULL,
    name VARCHAR NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL,
    balance INTEGER NOT NULL
);

CREATE TABLE stocks (
    id SERIAL PRIMARY KEY,
    name VARCHAR NOT NULL,
    into_market BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL,
    into_market_at TIMESTAMP NOT NULL
);

CREATE TABLE new_stocks (
    id SERIAL PRIMARY KEY REFERENCES stocks(id),
    offer_circ BIGINT NOT NULL,
    offer_price INTEGER NOT NULL,
    offer_unfulfilled BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE user_stock (
    user_id SERIAL REFERENCES users(id),
    stock_id SERIAL REFERENCES stocks(id),
    hold BIGINT NOT NULL,
    PRIMARY KEY (user_id, stock_id)
);

CREATE TABLE user_ask_entrusts ( -- 买入委托
    id SERIAL PRIMARY KEY,
    user_id SERIAL REFERENCES users(id),
    stock_id SERIAL REFERENCES stocks(id),
    price INTEGER NOT NULL,
    amount BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL
);
CREATE INDEX ask_entrusts_stock_id_and_price ON user_ask_entrusts(stock_id, price);

CREATE TABLE user_bid_entrusts ( -- 卖出委托
    id SERIAL PRIMARY KEY,
    user_id SERIAL REFERENCES users(id),
    stock_id SERIAL REFERENCES stocks(id),
    price INTEGER NOT NULL,
    amount BIGINT NOT NULL,
    fulfilled BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL
);
CREATE INDEX bid_entrusts_stock_id_and_price ON user_bid_entrusts(stock_id, price);

CREATE TABLE deals (
    id SERIAL PRIMARY KEY,
    buy_user_id SERIAL REFERENCES users(id),
    sell_user_id SERIAL REFERENCES users(id),
    stock_id SERIAL REFERENCES stocks(id),
    price INTEGER NOT NULL,
    amount BIGINT NOT NULL,
    created_at TIMESTAMP NOT NULL    -- 成交时间，不是委托时间
);
CREATE INDEX deals_stock_id_and_price ON user_bid_entrusts(stock_id, price);
