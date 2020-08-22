SELECT
	t.stock_id IS NOT NULL AS is_favorited
FROM (
	SELECT
		query_stock_id,
		t1.stock_id AS stock_id
	FROM
	unnest( $1 ) WITH ORDINALITY AS query(query_stock_id, ordinality)
	LEFT JOIN
	(
		SELECT
			DISTINCT ON (stock_id)
			user_fav_stock.stock_id AS stock_id
		FROM user_fav_stock
		WHERE user_fav_stock.user_id = ( $2 ) AND user_fav_stock.stock_id = ANY( $1 )
	) AS t1
	ON t1.stock_id = query_stock_id
	ORDER BY ordinality
) AS t;