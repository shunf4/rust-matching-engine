use crate::schema::*;

#[derive(Queryable, Insertable)]
#[table_name="users"]
pub struct User {
    pub id: i32,
    pub password_hashed: String,
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
    pub balance: i32
}

impl User {

}



#[derive(Queryable, Insertable)]
#[table_name="deals"]
pub struct Deals {
    pub id: i32,
    pub buy_user_id: i32,
    pub sell_user_id: i32,
    pub stock_id: i32,
    pub price: i32,
    pub amount: i64,
    pub created_at: chrono::NaiveDateTime
}

impl Deals {

}



#[derive(Queryable, Insertable, Serialize)]
#[table_name="stocks"]
pub struct Stocks {
    pub id: i32,
    pub issuer_id: i32,
    pub name: String,
    pub into_market: bool,
    pub into_market_at: Option<chrono::NaiveDateTime>,
}

impl Stocks {

}



#[derive(Queryable, Insertable)]
#[table_name="new_stocks"]
pub struct NewStocks {
    pub id: i32,
    pub offer_circ: i64,
    pub offer_price: i32,
    pub offer_unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime
}

impl NewStocks {

}




#[derive(Queryable, Insertable)]
#[table_name="user_ask_entrusts"]
pub struct AskEntrust {
    pub id: i32,
    pub user_id: i32,
    pub stock_id: i32,
    pub price: i32,
    pub amount: i64,
    pub created_at: chrono::NaiveDateTime
}

impl AskEntrust {

}



#[derive(Queryable, Insertable)]
#[table_name="user_bid_entrusts"]
pub struct BidEntrust {
    pub id: i32,
    pub user_id: i32,
    pub stock_id: i32,
    pub price: i32,
    pub amount: i64,
    pub created_at: chrono::NaiveDateTime
}

impl BidEntrust {

}




