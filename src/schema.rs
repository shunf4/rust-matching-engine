table! {
    deals (id) {
        id -> Int4,
        buy_user_id -> Int4,
        sell_user_id -> Int4,
        stock_id -> Int4,
        price -> Int4,
        amount -> Int8,
        created_at -> Timestamp,
    }
}

table! {
    new_stocks (id) {
        id -> Int4,
        offer_circ -> Int8,
        offer_price -> Int4,
        offer_unfulfilled -> Int8,
        created_at -> Timestamp,
    }
}

table! {
    stocks (id) {
        id -> Int4,
        issuer_id -> Int4,
        name -> Varchar,
        into_market -> Bool,
        into_market_at -> Nullable<Timestamp>,
    }
}

table! {
    user_ask_entrusts (id) {
        id -> Int4,
        user_id -> Int4,
        stock_id -> Int4,
        price -> Int4,
        amount -> Int8,
        created_at -> Timestamp,
    }
}

table! {
    user_bid_entrusts (id) {
        id -> Int4,
        user_id -> Int4,
        stock_id -> Int4,
        price -> Int4,
        amount -> Int8,
        fulfilled -> Int8,
        created_at -> Timestamp,
    }
}

table! {
    users (id) {
        id -> Int4,
        password_hashed -> Varchar,
        name -> Varchar,
        created_at -> Timestamp,
        balance -> Int4,
    }
}

table! {
    user_stock (user_id, stock_id) {
        user_id -> Int4,
        stock_id -> Int4,
        hold -> Int8,
    }
}

joinable!(deals -> stocks (stock_id));
joinable!(new_stocks -> stocks (id));
joinable!(stocks -> users (issuer_id));
joinable!(user_ask_entrusts -> stocks (stock_id));
joinable!(user_ask_entrusts -> users (user_id));
joinable!(user_bid_entrusts -> stocks (stock_id));
joinable!(user_bid_entrusts -> users (user_id));
joinable!(user_stock -> stocks (stock_id));
joinable!(user_stock -> users (user_id));

allow_tables_to_appear_in_same_query!(
    deals,
    new_stocks,
    stocks,
    user_ask_entrusts,
    user_bid_entrusts,
    users,
    user_stock,
);
