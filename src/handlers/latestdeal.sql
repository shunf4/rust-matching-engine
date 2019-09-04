SELECT
    price
FROM (
	SELECT
		query_stock_id,
		t1.*
	FROM
	unnest( $1 ) WITH ORDINALITY AS query(query_stock_id, ordinality)
	LEFT JOIN
	(
		SELECT
			DISTINCT ON (stock_id)
			deals.stock_id AS stock_id,
			deals.price AS price,
			MAX(deals.created_at)
				OVER (
					PARTITION BY deals.stock_id
				)
			AS ts
		FROM deals
		WHERE deals.stock_id = ANY( $2 )
	) AS t1
	ON t1.stock_id = query_stock_id
	ORDER BY ordinality
) AS t;