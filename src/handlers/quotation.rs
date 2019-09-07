use actix_web::{
    web, Error, HttpRequest, HttpResponse, FromRequest
};
use actix_web::error::BlockingError;
use actix_identity::Identity;
use crate::models::User;
use crate::models::NewStock;
use crate::models::Stock;
use crate::models::AskOrder;
use crate::models::BidOrder;
use crate::models::Deal;

use std::convert::TryFrom;
use std::convert::TryInto;

use futures::Future;
use crate::errors::EngineError;

use crate::common::Pool;
use diesel::PgConnection;
use diesel::prelude::*;

use std::str::FromStr;

use super::users::{RememberUserModel};
use super::PagingModel;

use crate::schema::*;

use diesel::sql_types;

#[derive(QueryableByName, Serialize, Deserialize)]
pub struct TimeIntervalQuotationModel {
    #[sql_type = "sql_types::Timestamp"]
    pub time: chrono::NaiveDateTime,
    #[sql_type = "sql_types::Nullable<sql_types::Double>"]
    pub price: Option<f64>,
}

#[derive(Queryable, Serialize, Deserialize)]
pub struct RecentDealQuotationModel {
    pub id: i64,
    pub buy_user_id: i64,
    pub sell_user_id: Option<i64>,  // 当是 NULL 时，表示是购买发行新股
    pub price: i32,
    pub amount: i64,
    pub created_at: chrono::NaiveDateTime
}

#[derive(QueryableByName, Serialize, Deserialize)]
pub struct OrderByPriceModel {
    #[sql_type = "sql_types::Int8"]
    pub amount: i64,
    #[sql_type = "sql_types::Int4"]
    pub price: i32,
}

#[derive(Serialize)]
pub struct QuotationModel {
    pub time_quote: Vec<TimeIntervalQuotationModel>,
    pub recent_deal: Vec<RecentDealQuotationModel>,
    pub ask_prices: Vec<OrderByPriceModel>,
    pub bid_prices: Vec<OrderByPriceModel>,
}

pub fn get_quotation(
    stock_id: web::Path<u64>,
    _: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_id = stock_id.into_inner();
    use futures::future::join_all;

    enum QueryType {
        Time,
        Deal,
        Ask,
        Bid
    }

    let get_block = |query: QueryType, pool: Pool| web::block(move || match query {
        QueryType::Time => get_timequote_query(stock_id, pool),
        QueryType::Deal => get_dealquote_query(stock_id, pool),
        QueryType::Ask => get_askquote_query(stock_id, pool),
        QueryType::Bid => get_bidquote_query(stock_id, pool)
    }).from_err();

    let pool = pool.into_inner();
    let pool = pool.as_ref();

    join_all(vec![
        get_block(QueryType::Time, pool.clone()),
        get_block(QueryType::Deal, pool.clone()),
        get_block(QueryType::Ask, pool.clone()),
        get_block(QueryType::Bid, pool.clone()),
    ]).then(
        move |res: Result<Vec<serde_json::Value>, BlockingError<EngineError>>|
            match res {
                Ok(mut m) => Ok(HttpResponse::Ok().json({
                        //debug!("{:?}", &m);
                        let bid_prices = serde_json::from_value(m.pop().unwrap()).map_err(|json_err| {
                                EngineError::InternalError(format!("内部 JSON 转换错误：{}", json_err))
                            })?;
                        let ask_prices = serde_json::from_value(m.pop().unwrap()).map_err(|json_err| {
                                EngineError::InternalError(format!("内部 JSON 转换错误：{}", json_err))
                            })?;
                        let recent_deal = serde_json::from_value(m.pop().unwrap()).map_err(|json_err| {
                                EngineError::InternalError(format!("内部 JSON 转换错误：{}", json_err))
                            })?;
                        let time_quote = serde_json::from_value(m.pop().unwrap()).map_err(|json_err| {
                                EngineError::InternalError(format!("内部 JSON 转换错误：{}", json_err))
                            })?;
                        QuotationModel {
                            time_quote,
                            recent_deal,
                            ask_prices,
                            bid_prices
                        }
                    }
                )),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_quotation_query(stock_id: u64, pool: web::Data<Pool>) -> Result<QuotationModel, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::new_stocks::dsl as newdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    // 分时

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    let query = diesel::sql_query(include_str!("timequote.sql"))
                    .bind::<sql_types::BigInt, _>(stock_id);

    debug!("Get time quote SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let timequotes = query.load::<TimeIntervalQuotationModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?;

    // 最近交易

    let query = dldsl::deals.filter(
                    dldsl::stock_id.eq(stock_id).and(
                        dldsl::sell_user_id.is_not_null()
                    )
                )
                .order_by(dldsl::created_at.desc())
                .select(
                    (dldsl::id, dldsl::buy_user_id, dldsl::sell_user_id, dldsl::price, dldsl::amount, dldsl::created_at)
                )
                .limit(5);

    debug!("Get deal quote SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let dealquotes = query.get_results::<RecentDealQuotationModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?;

    // 买卖委托

    let query = diesel::sql_query(include_str!("askquote.sql"))
                    .bind::<sql_types::BigInt, _>(stock_id);

    debug!("Get ask quote SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let askquotes = query.load::<OrderByPriceModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?;


    let query = diesel::sql_query(include_str!("bidquote.sql"))
                    .bind::<sql_types::BigInt, _>(stock_id);

    debug!("Get bid quote SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let bidquotes = query.load::<OrderByPriceModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?;

    Ok(QuotationModel {
        time_quote: timequotes,
        recent_deal: dealquotes,
        ask_prices: askquotes,
        bid_prices: bidquotes
    })
}

fn get_timequote_query(stock_id: u64, pool: Pool) -> Result<serde_json::Value, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::new_stocks::dsl as newdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    // 分时

    let query = diesel::sql_query(include_str!("timequote.sql"))
                    .bind::<sql_types::BigInt, _>(stock_id);

    debug!("Get time quote SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let model = query.load::<TimeIntervalQuotationModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?;

    serde_json::to_value(model).map_err(|json_err| {
        EngineError::InternalError(format!("内部 JSON 转换错误：{}", json_err))
    })
}

fn get_dealquote_query(stock_id: u64, pool: Pool) -> Result<serde_json::Value, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::new_stocks::dsl as newdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    // 最近交易

    let query = dldsl::deals.filter(
                    dldsl::stock_id.eq(stock_id).and(
                        dldsl::sell_user_id.is_not_null()
                    )
                )
                .order_by(dldsl::created_at.desc())
                .select(
                    (dldsl::id, dldsl::buy_user_id, dldsl::sell_user_id, dldsl::price, dldsl::amount, dldsl::created_at)
                )
                .limit(5);

    debug!("Get deal quote SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let model = query.get_results::<RecentDealQuotationModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?;

    serde_json::to_value(model).map_err(|json_err| {
        EngineError::InternalError(format!("内部 JSON 转换错误：{}", json_err))
    })
}

fn get_askquote_query(stock_id: u64, pool: Pool) -> Result<serde_json::Value, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::new_stocks::dsl as newdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    // 买卖委托

    let query = diesel::sql_query(include_str!("askquote.sql"))
                    .bind::<sql_types::BigInt, _>(stock_id);

    debug!("Get ask quote SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let model = query.load::<OrderByPriceModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?;

    serde_json::to_value(model).map_err(|json_err| {
        EngineError::InternalError(format!("内部 JSON 转换错误：{}", json_err))
    })
}

fn get_bidquote_query(stock_id: u64, pool: Pool) -> Result<serde_json::Value, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::new_stocks::dsl as newdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    let query = diesel::sql_query(include_str!("bidquote.sql"))
                    .bind::<sql_types::BigInt, _>(stock_id);

    debug!("Get bid quote SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let model = query.load::<OrderByPriceModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?;

    serde_json::to_value(model).map_err(|json_err| {
        EngineError::InternalError(format!("内部 JSON 转换错误：{}", json_err))
    })
}

//////////
#[derive(QueryableByName, Serialize)]
pub struct PriceModel {
    #[sql_type = "sql_types::Nullable<sql_types::Int4>"]
    pub price: Option<i32>,
}

pub fn get_prices(
    stock_ids: web::Path<String>,
    _: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_ids = stock_ids.into_inner();
   
    web::block(
        move || {
            let stock_ids: Vec<u64> = serde_json::from_str(&stock_ids[..]).map_err(|json_err| EngineError::BadRequest(format!("解析股票 ID 列表错误：{}", json_err)))?;
            get_prices_query(stock_ids, pool)
        }
    ).then(
        move |res: Result<Vec<PriceModel>, BlockingError<EngineError>>|
            match res {
                Ok(m) => Ok(HttpResponse::Ok().json(m)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_prices_query(stock_ids: Vec<u64>, pool: web::Data<Pool>) -> Result<Vec<PriceModel>, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::new_stocks::dsl as newdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let stock_ids = stock_ids.into_iter().map(|stock_id| i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))).collect::<Result<Vec<i64>, EngineError>>()?;

    let query = diesel::sql_query(include_str!("latestdeal.sql"))
                    .bind::<sql_types::Array<sql_types::BigInt>, _>(&stock_ids);

    debug!("Get prices SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.load::<PriceModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}
