SELECT
    price
FROM (
SELECT
    deals.price AS price,
    MAX(deals.created_at)
        OVER (
            PARTITION BY deals.stock_id
        )
    AS ts
FROM deals
WHERE
    deals.stock_id = ANY(?)
) AS t;