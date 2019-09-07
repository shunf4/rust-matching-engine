use crate::schema::*;

#[derive(Queryable, Insertable)]
#[table_name="users"]
pub struct User {
    pub id: i64,
    pub password_hashed: String,
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
    pub balance: i64
}

impl User {

}



#[derive(Queryable, Insertable)]
#[table_name="deals"]
pub struct Deal {
    pub id: i64,
    pub buy_user_id: i64,
    pub sell_user_id: Option<i64>,  // 当是 NULL 时，表示是购买发行新股
    pub stock_id: i64,
    pub price: i32,
    pub amount: i64,
    pub created_at: chrono::NaiveDateTime
}

impl Deal {

}



#[derive(Queryable, Insertable, Serialize)]
#[table_name="stocks"]
pub struct Stock {
    pub id: i64,
    pub name: String,
    pub into_market: bool,
    pub into_market_at: Option<chrono::NaiveDateTime>,
}

impl Stock {

}



#[derive(Queryable, Insertable, AsChangeset, Identifiable)]
#[table_name="new_stocks"]
pub struct NewStock {
    pub id: i64,
    pub issuer_id: i64,
    pub offer_circ: i64,
    pub offer_price: i32,
    pub offer_unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime
}

impl NewStock {

}




#[derive(Queryable, Insertable, AsChangeset, Identifiable)]
#[table_name="user_ask_orders"]
pub struct AskOrder {
    pub id: i64,
    pub user_id: i64,
    pub stock_id: i64,
    pub price: i32,
    pub volume: i64,
    pub unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime
}

impl AskOrder {

}



#[derive(Queryable, Insertable, AsChangeset, Identifiable)]
#[table_name="user_bid_orders"]
pub struct BidOrder {
    pub id: i64,
    pub user_id: i64,
    pub stock_id: i64,
    pub price: i32,
    pub volume: i64,
    pub unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime
}

impl BidOrder {

}




