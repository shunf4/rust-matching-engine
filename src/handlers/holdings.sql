SELECT
	COALESCE(t.hold, 0) AS hold
FROM (
	SELECT
		query_stock_id,
		t1.stock_id AS stock_id,
		t1.hold AS hold
	FROM
		unnest( $1 ) WITH ORDINALITY AS query(query_stock_id, ordinality)
	LEFT JOIN
	(
		SELECT
			DISTINCT ON (stock_id)
			user_hold_stock.stock_id AS stock_id,
			user_hold_stock.hold AS hold
		FROM user_hold_stock
		WHERE user_hold_stock.user_id = ( $2 ) AND user_hold_stock.stock_id = ANY( $1 )
	) AS t1
	ON t1.stock_id = query_stock_id
	ORDER BY ordinality
) AS t;