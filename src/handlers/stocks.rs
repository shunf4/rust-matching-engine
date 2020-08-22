use actix_web::{
    web, Error, HttpRequest, HttpResponse, FromRequest
};
use actix_web::error::BlockingError;
use actix_identity::Identity;
use crate::models::Stock;

use std::convert::TryFrom;
use std::convert::TryInto;

use futures::Future;
use crate::errors::EngineError;

use crate::common::Pool;
use std::sync::Arc;
use diesel::PgConnection;
use diesel::prelude::*;

use std::str::FromStr;

use super::users::{RememberUserModel};
use diesel::sql_types;

pub fn make_scope() -> actix_web::Scope {
    web::scope("/stocks")
        .service(
            web::resource("/")  // Scope 会自动加尾 /，所以 /stocks 无法匹配
                .route(web::get().to_async(get_stocks))     // 查询已上市股票
                .route(web::post().to_async(ipo_stock))   // 新股发行
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            super::favorite::make_scope()
        )
        .service(
            web::resource("/{ids}/prices")  // Scope 会自动加尾 /，所以 /stocks 无法匹配
                .route(web::get().to_async(super::quotation::get_prices))     // 查询一系列股票的实时交易价格
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/ipo/")  // Scope 会自动加尾 /，所以 /stocks 无法匹配
                .route(web::get().to_async(get_ipo_stocks))     // 查询未上市股票
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/my/")  // Scope 会自动加尾 /，所以 /stocks 无法匹配
                .route(web::get().to_async(get_my_stocks))     // 查询自己的股票（上市）
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/my/holds/")  // Scope 会自动加尾 /，所以 /stocks 无法匹配
                .route(web::get().to_async(get_my_holds))     // 查询自己持有的股票
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/{ids}/holding")
                .route(web::get().to_async(get_stocks_holding))     // 查询一系列股票的持有情况
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/my/ipo/")  // Scope 会自动加尾 /，所以 /stocks 无法匹配
                .route(web::get().to_async(get_my_ipo_stocks))     // 查询自己的股票（未上市）
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/{id}/quotation")
                .route(web::get().to_async(super::quotation::get_quotation))      // 查看行情
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/{id}")
                .route(web::get().to_async(get_stock))      // 获取股票
                .route(web::method(http::Method::from_str("LIST").unwrap()).to_async(list_stock))      // 上市股票
                .route(web::method(http::Method::from_str("IPOBUY").unwrap()).to_async(super::orders::ipo_buy))      // 购买未上市股票
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/by-name/{name}")
                .route(web::get().to_async(get_stock_by_name))      // 获取股票
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )

}

#[derive(Debug, Deserialize, Clone)]
pub struct IPOModel {
    pub name: String,
    pub offer_circ: i64,
    pub offer_price: i32,
}

use crate::schema::*;
#[derive(Debug, Deserialize, Insertable)]
#[table_name="new_stocks"]
pub struct IPONewStockModel {
    pub id: i64,
    pub issuer_id: i64,
    pub offer_circ: i64,
    pub offer_price: i32,
    pub created_at: chrono::NaiveDateTime,
    pub offer_unfulfilled: i64,
}

impl IPONewStockModel {
    fn from_borrowed_ipo_and_id_and_user(ipo: &IPOModel, user: &RememberUserModel, id: i64) -> IPONewStockModel {
        IPONewStockModel {
            id,
            issuer_id: user.id,
            offer_circ: ipo.offer_circ,
            offer_price: ipo.offer_price,
            offer_unfulfilled: ipo.offer_circ,
            created_at: chrono::Utc::now().naive_utc(),
        }
    }
}

use crate::schema::*;
#[derive(Debug, Deserialize, Insertable)]
#[table_name="stocks"]
pub struct IPOStockModel {
    pub name: String,
    pub into_market: bool,
}

impl IPOStockModel {
    fn from_ipo(ipo: &IPOModel) -> IPOStockModel {
        IPOStockModel {
            name: ipo.name.to_owned(),
            into_market: false,
        }
    }
}

///////////////

pub fn ipo_stock(
    ipo: web::Json<IPOModel>,
    curr_user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
   
    web::block(
        move || {
            ipo_stock_query(ipo.into_inner(), curr_user, pool)
        }
    ).then(
        move |res: Result<(), BlockingError<EngineError>>|
            match res {
                Ok(_) => Ok(HttpResponse::Ok().finish()),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn ipo_stock_query(ipo: IPOModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<(), EngineError> {
    use crate::schema::stocks::dsl::*;
    use crate::schema::new_stocks::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    conn.transaction(|| {
        // 保证原子性
        // 第一步：建立 stock，有名字重复则马上失败
        let query_stock = diesel::insert_into(stocks)
            .values(IPOStockModel::from_ipo(&ipo));

        debug!("New stock stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_stock));

        let new_id = query_stock.get_result::<Stock>(conn)
            .optional()
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库插入股票错误，可能是股票重名所致：{}", db_err))
            })?
            .ok_or_else(|| EngineError::InternalError(format!("数据库插入股票后无返回值错误")))?
            .id;

        // 第二步：建立 ipo_stock
        let query_ipo_stock = diesel::insert_into(new_stocks)
            .values(IPONewStockModel::from_borrowed_ipo_and_id_and_user(&ipo, &user, new_id));

        debug!("New stock new_stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_ipo_stock));

        let affected = query_ipo_stock.execute(conn)
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库插入未上市股票错误：{}", db_err))
            })?;

        match affected {
            1 => Ok(()),
            _ => Err(EngineError::InternalError(format!("数据库插入上市并非 1 行：{}", affected)))
        }
    })
}


/////////////


pub fn list_stock(
    stock_id: web::Path<u64>,
    curr_user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_id = stock_id.into_inner();
   
    web::block(
        move || {
            list_stock_query(stock_id, curr_user, pool)
        }
    ).then(
        move |res: Result<(), BlockingError<EngineError>>|
            match res {
                Ok(_) => Ok(HttpResponse::Ok().finish()),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn list_stock_query(stock_id: u64, curr_user: RememberUserModel, pool: web::Data<Pool>) -> Result<(), EngineError> {
    use crate::schema::stocks::dsl::*;
    use crate::schema::new_stocks::dsl::*;

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    // 第一步：验证此 stock 是用户本人发行
    let query_target = stocks.inner_join(new_stocks)
                            .filter(
                                crate::schema::new_stocks::dsl::id.eq(stock_id).and(
                                    issuer_id.eq(curr_user.id)
                                ).and(
                                    into_market.eq(false)
                                )
                            );
    let query_check_issuer = query_target.select(crate::schema::new_stocks::dsl::id);

    debug!("List stock check_issuer SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_check_issuer));

    query_check_issuer
        .get_result::<i64>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::BadRequest(format!("没有这只股票，或这只股票不是你发行的。")))?;

    // 第二步：上市
    let query_list_stock = diesel::update(stocks.filter(
                                crate::schema::stocks::dsl::id.eq(stock_id)
                            ))
                            .set((
                                    into_market.eq(true),
                                    into_market_at.eq(chrono::Utc::now().naive_utc())
                            ));

    debug!("List stock list_stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_list_stock));

    query_list_stock.get_result::<(i64, String, bool, Option<chrono::NaiveDateTime>)>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库插入上市股票错误：{}", db_err))
        })?;

    Ok(())
}



/////////////
#[derive(Debug, Deserialize, Clone)]
pub enum PagingOrder {
    Alphabetical,
    Latest
}

#[derive(Debug, Deserialize, Clone)]
pub struct PagingModel {
    pub offset: Option<u64>,
    pub limit: Option<u64>,
    pub order: Option<PagingOrder>
}

#[derive(Queryable, Serialize)]
pub struct GetStockModel {
    pub id: i64,
    pub name: String,
    pub into_market_at: Option<chrono::NaiveDateTime>,
}

pub fn get_stocks(
    paging: web::Query<PagingModel>,
    _: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_stocks_query(paging, pool)
        }
    ).then(
        move |res: Result<Vec<GetNewStockModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_stocks_query(paging: PagingModel, pool: web::Data<Pool>) -> Result<Vec<GetNewStockModel>, EngineError> {
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;
    use crate::schema::new_stocks::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = new_stocks.inner_join(users).inner_join(stocks).filter(
                    into_market.eq(true)
                )
                    .order(into_market_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (crate::schema::stocks::dsl::id, crate::schema::stocks::dsl::name, crate::schema::users::dsl::id.nullable(),crate::schema::users::dsl::name.nullable(), into_market, into_market_at, offer_circ.nullable(), offer_price.nullable(), offer_unfulfilled.nullable(), crate::schema::new_stocks::dsl::created_at.nullable())
                    );

    debug!("Get stocks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<GetNewStockModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}


/////////////

#[derive(Queryable, Serialize)]
pub struct GetNewStockModelNotNull {
    pub id: i64,
    pub issuer_name: String,
    pub name: String,
    pub into_market: bool,
    pub into_market_at: Option<chrono::NaiveDateTime>,
    pub offer_circ: i64,
    pub offer_price: i32,
    pub offer_unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime
}

#[derive(Queryable, Serialize)]
pub struct GetNewStockModel {
    pub id: i64,
    pub name: String,
    pub issuer_id: Option<i64>,
    pub issuer_name: Option<String>,
    pub into_market: bool,
    pub into_market_at: Option<chrono::NaiveDateTime>,
    pub offer_circ: Option<i64>,
    pub offer_price: Option<i32>,
    pub offer_unfulfilled: Option<i64>,
    pub created_at: Option<chrono::NaiveDateTime>
}

pub fn get_ipo_stocks(
    paging: web::Query<PagingModel>,
    _: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_ipo_stocks_query(paging, pool)
        }
    ).then(
        move |res: Result<Vec<GetNewStockModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_ipo_stocks_query(paging: PagingModel, pool: web::Data<Pool>) -> Result<Vec<GetNewStockModel>, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = new_stocks.inner_join(stocks).inner_join(users).filter(
                    into_market.eq(false)
                )
                    .order(crate::schema::new_stocks::dsl::created_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (crate::schema::stocks::dsl::id, crate::schema::stocks::dsl::name, crate::schema::users::dsl::id.nullable(),crate::schema::users::dsl::name.nullable(), into_market, into_market_at, offer_circ.nullable(), offer_price.nullable(), offer_unfulfilled.nullable(), crate::schema::new_stocks::dsl::created_at.nullable())
                    );

    debug!("Get ipo stocks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<GetNewStockModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}

/////////////

pub fn get_my_stocks(
    paging: web::Query<PagingModel>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_my_stocks_query(paging, user, pool)
        }
    ).then(
        move |res: Result<Vec<GetNewStockModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_my_stocks_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<GetNewStockModel>, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = new_stocks.inner_join(users).inner_join(stocks).filter(
                    into_market.eq(true).and(
                        issuer_id.eq(user.id)
                    )
                )
                    .order(into_market_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (crate::schema::stocks::dsl::id, crate::schema::stocks::dsl::name, crate::schema::users::dsl::id.nullable(),crate::schema::users::dsl::name.nullable(), into_market, into_market_at, offer_circ.nullable(), offer_price.nullable(), offer_unfulfilled.nullable(), crate::schema::new_stocks::dsl::created_at.nullable())
                    );

    debug!("Get my stocks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<GetNewStockModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}

/////////////

pub fn get_my_holds(
    paging: web::Query<PagingModel>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_my_holds_query(paging, user, pool)
        }
    ).then(
        move |res: Result<Vec<GetNewStockModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_my_holds_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<GetNewStockModel>, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::new_stocks::dsl as newdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = reldsl::user_hold_stock.inner_join(stkdsl::stocks.left_join(newdsl::new_stocks)).inner_join(usrdsl::users).filter(
                    usrdsl::id.eq(user.id)
                )
                    .order(reldsl::updated_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (stkdsl::id, stkdsl::name, usrdsl::id.nullable(), usrdsl::name.nullable(), stkdsl::into_market, stkdsl::into_market_at, newdsl::offer_circ.nullable(), newdsl::offer_price.nullable(), newdsl::offer_unfulfilled.nullable(), newdsl::created_at.nullable())
                    );

    debug!("Get my holds SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<GetNewStockModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}

//////////////////

pub fn get_my_ipo_stocks(
    paging: web::Query<PagingModel>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_my_ipo_stocks_query(paging, user, pool)
        }
    ).then(
        move |res: Result<Vec<GetNewStockModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_my_ipo_stocks_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<GetNewStockModel>, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = new_stocks.inner_join(users).inner_join(stocks).filter(
                    into_market.eq(false).and(
                        issuer_id.eq(user.id)   
                    )
                )
                    .order(crate::schema::new_stocks::dsl::created_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (crate::schema::stocks::dsl::id, crate::schema::stocks::dsl::name, crate::schema::users::dsl::id.nullable(),crate::schema::users::dsl::name.nullable(), into_market, into_market_at, offer_circ.nullable(), offer_price.nullable(), offer_unfulfilled.nullable(), crate::schema::new_stocks::dsl::created_at.nullable())
                    );

    debug!("Get my ipo stocks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<GetNewStockModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}


//////////////////

pub fn get_stock(
    stock_id: web::Path<u64>,
    _: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_id = stock_id.into_inner();
   
    web::block(
        move || {
            get_stock_query(stock_id, pool)
        }
    ).then(
        move |res: Result<GetNewStockModel, BlockingError<EngineError>>|
            match res {
                Ok(stock) => Ok(HttpResponse::Ok().json(stock)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_stock_query(stock_id: u64, pool: web::Data<Pool>) -> Result<GetNewStockModel, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = stocks.left_join(new_stocks.inner_join(users)).filter(
                    crate::schema::stocks::dsl::id.eq(stock_id)
                )
                .select(
                    (crate::schema::stocks::dsl::id, crate::schema::stocks::dsl::name, crate::schema::users::dsl::id.nullable(),crate::schema::users::dsl::name.nullable(), into_market, into_market_at, offer_circ.nullable(), offer_price.nullable(), offer_unfulfilled.nullable(), crate::schema::new_stocks::dsl::created_at.nullable())
                );

    debug!("Get stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_result::<GetNewStockModel>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::NotFound(format!("没有这只股票。")))
}


//////////////////

pub fn get_stock_by_name(
    stock_name: web::Path<String>,
    _: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_name = stock_name.into_inner();
   
    web::block(
        move || {
            get_stock_by_name_query(stock_name, pool)
        }
    ).then(
        move |res: Result<GetNewStockModel, BlockingError<EngineError>>|
            match res {
                Ok(stock) => Ok(HttpResponse::Ok().json(stock)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_stock_by_name_query(stock_name: String, pool: web::Data<Pool>) -> Result<GetNewStockModel, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = new_stocks.inner_join(users).inner_join(stocks).filter(
                    crate::schema::stocks::dsl::name.eq(stock_name)
                )
                .select(
                    (crate::schema::stocks::dsl::id, crate::schema::stocks::dsl::name, crate::schema::users::dsl::id.nullable(),crate::schema::users::dsl::name.nullable(), into_market, into_market_at, offer_circ.nullable(), offer_price.nullable(), offer_unfulfilled.nullable(), crate::schema::new_stocks::dsl::created_at.nullable())
                );

    debug!("Get stock by name SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_result::<GetNewStockModel>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::NotFound(format!("没有这只股票。")))
}


/////////////
#[derive(QueryableByName, Serialize)]
pub struct HoldingModel {
    #[sql_type = "sql_types::BigInt"]
    pub hold: i64
} 

pub fn get_stocks_holding(
    stock_ids: web::Path<String>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_ids = stock_ids.into_inner();
   
    web::block(
        move || {
            let stock_ids: Vec<u64> = serde_json::from_str(&stock_ids[..]).map_err(|json_err| EngineError::BadRequest(format!("解析股票 ID 列表错误：{}", json_err)))?;
            get_stocks_holding_query(stock_ids, user, pool)
        }
    ).then(
        move |res: Result<Vec<HoldingModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_stocks_holding_query(stock_ids: Vec<u64>, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<HoldingModel>, EngineError> {
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

    let query = diesel::sql_query(include_str!("holdings.sql"))
                    .bind::<sql_types::Array<sql_types::BigInt>, _>(&stock_ids)
                    .bind::<sql_types::BigInt, _>(&user.id);

    debug!("Get stocks holding SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.load::<HoldingModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}