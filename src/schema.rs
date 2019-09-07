table! {
    deals (id) {
        id -> Int8,
        buy_user_id -> Int8,
        sell_user_id -> Nullable<Int8>,
        stock_id -> Int8,
        price -> Int4,
        amount -> Int8,
        created_at -> Timestamp,
    }
}

table! {
    new_stocks (id) {
        id -> Int8,
        issuer_id -> Int8,
        offer_circ -> Int8,
        offer_price -> Int4,
        offer_unfulfilled -> Int8,
        created_at -> Timestamp,
    }
}

table! {
    stocks (id) {
        id -> Int8,
        name -> Varchar,
        into_market -> Bool,
        into_market_at -> Nullable<Timestamp>,
    }
}

table! {
    user_ask_orders (id) {
        id -> Int8,
        user_id -> Int8,
        stock_id -> Int8,
        price -> Int4,
        volume -> Int8,
        unfulfilled -> Int8,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    user_bid_orders (id) {
        id -> Int8,
        user_id -> Int8,
        stock_id -> Int8,
        price -> Int4,
        volume -> Int8,
        unfulfilled -> Int8,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    user_fav_stock (user_id, stock_id) {
        user_id -> Int8,
        stock_id -> Int8,
        created_at -> Timestamp,
    }
}

table! {
    user_hold_stock (user_id, stock_id) {
        user_id -> Int8,
        stock_id -> Int8,
        hold -> Int8,
        updated_at -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Int8,
        password_hashed -> Varchar,
        name -> Varchar,
        created_at -> Timestamp,
        balance -> Int8,
    }
}

joinable!(deals -> stocks (stock_id));
joinable!(new_stocks -> stocks (id));
joinable!(new_stocks -> users (issuer_id));
joinable!(user_ask_orders -> stocks (stock_id));
joinable!(user_ask_orders -> users (user_id));
joinable!(user_bid_orders -> stocks (stock_id));
joinable!(user_bid_orders -> users (user_id));
joinable!(user_fav_stock -> stocks (stock_id));
joinable!(user_fav_stock -> users (user_id));
joinable!(user_hold_stock -> stocks (stock_id));
joinable!(user_hold_stock -> users (user_id));

allow_tables_to_appear_in_same_query!(
    deals,
    new_stocks,
    stocks,
    user_ask_orders,
    user_bid_orders,
    user_fav_stock,
    user_hold_stock,
    users,
);
