SELECT
	deals.id AS id,
	stocks.id AS stock_id,
	stocks.name AS stock_name,
	buy_users.id AS buy_user_id,
	buy_users.name AS buy_user_name,
	sell_users.id AS sell_user_id,
	sell_users.name AS sell_user_name,
	price,
	amount,
	deals.created_at AS created_at
FROM
	deals
		INNER JOIN
	users AS buy_users ON deals.buy_user_id = buy_users.id
		INNER JOIN
	users AS sell_users ON deals.sell_user_id = sell_users.id
		INNER JOIN
	stocks ON deals.stock_id = stocks.id
WHERE
	buy_users.id = $1
		OR
	sell_users.id = $1
ORDER BY
	deals.created_at DESC
LIMIT $3 OFFSET $2;