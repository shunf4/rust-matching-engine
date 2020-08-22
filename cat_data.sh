#!/bin/bash
psql -P pager=no -f viewdatabase.sql postgresql://stock:stockengine@localhost/matching_engine_db

