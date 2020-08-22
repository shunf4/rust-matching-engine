SELECT id AS user_id, name, created_at, balance FROM users LIMIT 1000;

SELECT id AS stock_id, name AS stock_name, into_market, into_market_at FROM stocks LIMIT 1000;

SELECT
stocks.id AS stock_id,
stocks.name AS stock_name,
into_market,
into_market_at,
issuer_id,
offer_circ,
offer_price,
offer_unfulfilled,
created_at
FROM new_stocks RIGHT JOIN stocks ON new_stocks.id = stocks.id LIMIT 1000;

SELECT
users.id AS hold_user_id,
users.name AS user_name,
stocks.id AS stock_id,
stocks.name AS stock_name,
hold,
user_hold_stock.updated_at AS updated_at
FROM user_hold_stock INNER JOIN users ON user_hold_stock.user_id = users.id INNER JOIN stocks ON user_hold_stock.stock_id = stocks.id LIMIT 1000;

SELECT
users.id AS fav_user_id,
users.name AS user_name,
stocks.id AS stock_id,
stocks.name AS stock_name,
user_fav_stock.created_at AS created_at
FROM user_fav_stock INNER JOIN users ON user_fav_stock.user_id = users.id INNER JOIN stocks ON user_fav_stock.stock_id = stocks.id LEFT JOIN new_stocks ON new_stocks.id = stocks.id LIMIT 1000;

SELECT
user_ask_orders.id AS ask_id,
users.id AS user_id,
users.name AS user_name,
stocks.id AS stock_id,
stocks.name AS stock_name,
price,
volume,
unfulfilled,
user_ask_orders.created_at,
user_ask_orders.updated_at
FROM user_ask_orders INNER JOIN users ON user_ask_orders.user_id = users.id INNER JOIN stocks ON user_ask_orders.stock_id = stocks.id LIMIT 1000;

SELECT
user_bid_orders.id AS bid_id,
users.id AS user_id,
users.name AS user_name,
stocks.id AS stock_id,
stocks.name AS stock_name,
price,
volume,
unfulfilled,
user_bid_orders.created_at,
user_bid_orders.updated_at
FROM user_bid_orders INNER JOIN users ON user_bid_orders.user_id = users.id INNER JOIN stocks ON user_bid_orders.stock_id = stocks.id LIMIT 1000;

SELECT
deals.id AS deal_id,
buy_users.id AS buy_user_id,
buy_users.name AS buy_user_name,
sell_users.id AS sell_user_id,
sell_users.name AS sell_user_name,
stocks.id AS stock_id,
stocks.name AS stock_name,
price,
amount,
deals.created_at
FROM deals INNER JOIN users AS buy_users ON deals.buy_user_id = buy_users.id LEFT JOIN users AS sell_users ON deals.sell_user_id = sell_users.id INNER JOIN stocks ON deals.stock_id = stocks.id LIMIT 1000;
