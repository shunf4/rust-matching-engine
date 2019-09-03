SELECT t1.ts as time, SUM(price*amount)::FLOAT/SUM(amount) as price
FROM
generate_series(
    DATE_TRUNC('minute', CURRENT_TIMESTAMP) - INTERVAL '30 minutes',
    DATE_TRUNC('minute', CURRENT_TIMESTAMP) - INTERVAL '0 minutes',
    '1 minute'::interval
) AS t1(ts)
LEFT JOIN deals
ON
    t1.ts = DATE_TRUNC('minute', deals.created_at)
        AND
    stock_id = ?
GROUP BY t1.ts