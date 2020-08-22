SELECT
    SUM(user_ask_orders.unfulfilled) AS amount,
    user_ask_orders.price AS price
FROM user_ask_orders
WHERE
    user_ask_orders.stock_id = $1
        AND
    user_ask_orders.unfulfilled != 0
GROUP BY
    user_ask_orders.price
ORDER BY
    user_ask_orders.price DESC
LIMIT 5;