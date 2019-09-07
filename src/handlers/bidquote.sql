SELECT
    SUM(user_bid_orders.unfulfilled) AS amount,
    user_bid_orders.price AS price
FROM user_bid_orders
WHERE
    user_bid_orders.stock_id = $1
        AND
    user_bid_orders.unfulfilled != 0
GROUP BY
    user_bid_orders.price
ORDER BY
    user_bid_orders.price ASC
LIMIT 5;