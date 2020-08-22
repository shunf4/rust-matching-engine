--
-- PostgreSQL database dump
--

-- Dumped from database version 11.5 (Ubuntu 11.5-1.pgdg16.04+1)
-- Dumped by pg_dump version 11.5 (Ubuntu 11.5-1.pgdg16.04+1)



--
-- Data for Name: stocks; Type: TABLE DATA; Schema: public; Owner: stock
--

COPY public.stocks (id, name, into_market, into_market_at) FROM stdin;
4	冯舜控股2	f	\N
5	冯舜控股3	f	\N
6	冯舜控股4	f	\N
7	冯舜控股5	f	\N
12	张三控股	f	\N
1	冯舜控股	t	2019-09-04 14:41:41.060677
\.


--
-- Data for Name: users; Type: TABLE DATA; Schema: public; Owner: stock
--

COPY public.users (id, password_hashed, name, created_at, balance) FROM stdin;
2	BEEE7FCBE02FDF39F23F5CE65D84BD7D9C5FBE174168AE359CAFDA37B5F19D02	zhangsan	2019-09-04 14:31:44.079765	200000
1	BEEE7FCBE02FDF39F23F5CE65D84BD7D9C5FBE174168AE359CAFDA37B5F19D02	shunf4	2019-09-03 16:47:40.717321	10000
\.


--
-- Data for Name: deals; Type: TABLE DATA; Schema: public; Owner: stock
--

COPY public.deals (id, buy_user_id, sell_user_id, stock_id, price, amount, created_at) FROM stdin;
1	2	\N	1	900	100	2019-09-04 14:37:30.951548
2	2	\N	1	900	1900	2019-09-04 14:38:26.790477
3	2	\N	1	900	0	2019-09-04 14:38:33.932277
4	1	\N	4	900	100	2019-09-04 15:44:21.353315
\.


--
-- Data for Name: new_stocks; Type: TABLE DATA; Schema: public; Owner: stock
--

COPY public.new_stocks (id, issuer_id, offer_circ, offer_price, offer_unfulfilled, created_at) FROM stdin;
5	1	2000	900	2000	2019-09-04 14:26:17.043077
6	1	2000	900	2000	2019-09-04 14:26:21.623254
7	1	2000	900	2000	2019-09-04 14:26:24.929871
12	2	2000	900	2000	2019-09-04 14:32:27.801802
1	1	2000	900	0	2019-09-04 14:25:45.834794
4	1	2000	900	1900	2019-09-04 14:26:13.97188
\.


--
-- Data for Name: user_ask_orders; Type: TABLE DATA; Schema: public; Owner: stock
--

COPY public.user_ask_orders (id, user_id, stock_id, price, volume, unfulfilled, created_at) FROM stdin;
\.


--
-- Data for Name: user_bid_orders; Type: TABLE DATA; Schema: public; Owner: stock
--

COPY public.user_bid_orders (id, user_id, stock_id, price, volume, unfulfilled, created_at) FROM stdin;
\.


--
-- Data for Name: user_fav_stock; Type: TABLE DATA; Schema: public; Owner: stock
--

COPY public.user_fav_stock (user_id, stock_id, created_at) FROM stdin;
1	4	2019-09-05 08:47:24.811437
\.


--
-- Data for Name: user_hold_stock; Type: TABLE DATA; Schema: public; Owner: stock
--

COPY public.user_hold_stock (user_id, stock_id, hold) FROM stdin;
2	1	2000
1	4	100
\.


--
-- Name: deals_buy_user_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.deals_buy_user_id_seq', 1, false);


--
-- Name: deals_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.deals_id_seq', 4, true);


--
-- Name: deals_stock_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.deals_stock_id_seq', 1, false);


--
-- Name: new_stocks_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.new_stocks_id_seq', 1, false);


--
-- Name: new_stocks_issuer_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.new_stocks_issuer_id_seq', 1, false);


--
-- Name: stocks_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.stocks_id_seq', 12, true);


--
-- Name: user_ask_orders_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_ask_orders_id_seq', 1, false);


--
-- Name: user_ask_orders_stock_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_ask_orders_stock_id_seq', 1, false);


--
-- Name: user_ask_orders_user_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_ask_orders_user_id_seq', 1, false);


--
-- Name: user_bid_orders_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_bid_orders_id_seq', 1, false);


--
-- Name: user_bid_orders_stock_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_bid_orders_stock_id_seq', 1, false);


--
-- Name: user_bid_orders_user_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_bid_orders_user_id_seq', 1, false);


--
-- Name: user_fav_stock_stock_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_fav_stock_stock_id_seq', 1, false);


--
-- Name: user_fav_stock_user_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_fav_stock_user_id_seq', 1, false);


--
-- Name: user_hold_stock_stock_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_hold_stock_stock_id_seq', 1, false);


--
-- Name: user_hold_stock_user_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.user_hold_stock_user_id_seq', 1, false);


--
-- Name: users_id_seq; Type: SEQUENCE SET; Schema: public; Owner: stock
--

SELECT pg_catalog.setval('public.users_id_seq', 2, true);


--
-- PostgreSQL database dump complete
--

