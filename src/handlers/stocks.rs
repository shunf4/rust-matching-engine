use actix_web::{
    web, Error, HttpRequest, HttpResponse, FromRequest
};
use actix_web::error::BlockingError;
use actix_identity::Identity;
use crate::models::Stocks;

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

pub fn make_scope() -> actix_web::Scope {
    web::scope("/stocks")
        .service(
            web::resource("/")  // Scope 会自动加尾 /，所以 /stocks 无法匹配
                .route(web::get().to_async(get_stocks))     // 查询已上市股票
                .route(web::post().to_async(ipo_stock))   // 新股发行
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
            web::resource("/my/ipo/")  // Scope 会自动加尾 /，所以 /stocks 无法匹配
                .route(web::get().to_async(get_my_ipo_stocks))     // 查询自己的股票（未上市）
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/{id}")
                .route(web::get().to_async(get_stock))      // 获取股票
                .route(web::method(http::Method::from_str("LIST").unwrap()).to_async(list_stock))      // 上市股票
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
    pub id: i32,
    pub offer_circ: i64,
    pub offer_price: i32,
    pub created_at: chrono::NaiveDateTime,
    pub offer_unfulfilled: i64,
}

impl IPONewStockModel {
    fn from_borrowed_ipo_and_id(ipo: &IPOModel, id: i32) -> IPONewStockModel {
        IPONewStockModel {
            id,
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
    pub issuer_id: i32,
    pub into_market: bool,
}

impl IPOStockModel {
    fn from_ipo_and_user(ipo: &IPOModel, user: &RememberUserModel) -> IPOStockModel {
        IPOStockModel {
            name: ipo.name.to_owned(),
            issuer_id: user.id,
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
            .values(IPOStockModel::from_ipo_and_user(&ipo, &user));

        debug!("New stock stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_stock));

        let new_id = query_stock.get_result::<Stocks>(conn)
            .optional()
            .map_err(|db_err| {
                debug!("Database insert error when registering: {}", db_err);
                EngineError::InternalError(format!("数据库插入股票错误，可能是股票重名所致：{}", db_err))
            })?
            .ok_or_else(|| EngineError::InternalError(format!("数据库插入股票后无返回值错误")))?
            .id;

        // 第二步：建立 ipo_stock
        let query_ipo_stock = diesel::insert_into(new_stocks)
            .values(IPONewStockModel::from_borrowed_ipo_and_id(&ipo, new_id));

        debug!("New stock new_stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_ipo_stock));

        query_ipo_stock.execute(conn)
            .map_err(|db_err| {
                debug!("Database insert error when registering: {}", db_err);
                EngineError::InternalError(format!("数据库插入未上市股票错误：{}", db_err))
            })?;

        Ok(())
    })
}


/////////////


pub fn list_stock(
    stock_id: web::Path<u32>,
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

fn list_stock_query(stock_id: u32, curr_user: RememberUserModel, pool: web::Data<Pool>) -> Result<(), EngineError> {
    use crate::schema::stocks::dsl::*;
    use crate::schema::new_stocks::dsl::*;

    let stock_id = i32::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 32 字节有符号整数：{}。", try_err)))?;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    // 第一步：验证此 stock 是用户本人发行
    let query_target = stocks
                            .filter(
                                crate::schema::stocks::dsl::id.eq(stock_id).and(
                                    issuer_id.eq(curr_user.id)
                                )
                            );
    let query_check_issuer = query_target.select(crate::schema::stocks::dsl::id);

    debug!("List stock check_issuer SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_check_issuer));

    query_check_issuer
        .get_result::<i32>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::BadRequest(format!("没有这只股票，或这只股票不是你发行的。")))?;

    // 第二步：上市
    let query_list_stock = diesel::update(query_target)
                            .set((
                                    into_market.eq(true),
                                    into_market_at.eq(chrono::Utc::now().naive_utc())
                            ));

    debug!("List stock list_stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_list_stock));

    query_list_stock.get_result::<(i32, i32, String, bool, Option<chrono::NaiveDateTime>)>(conn)
        .map_err(|db_err| {
            debug!("Database insert error when registering: {}", db_err);
            EngineError::InternalError(format!("数据库插入上市股票错误：{}", db_err))
        })?;

    Ok(())
}



/////////////

#[derive(Debug, Deserialize, Clone)]
pub struct PagingModel {
    pub offset: Option<u32>,
    pub limit: Option<u32>
}

#[derive(Queryable, Serialize)]
pub struct GetStocksModel {
    pub id: i32,
    pub issuer_name: String,
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
        move |res: Result<Vec<GetStocksModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_stocks_query(paging: PagingModel, pool: web::Data<Pool>) -> Result<Vec<GetStocksModel>, EngineError> {
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = stocks.inner_join(users).filter(
                    into_market.eq(true)
                )
                    .order(into_market_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (crate::schema::stocks::dsl::id, crate::schema::users::dsl::name, crate::schema::stocks::dsl::name, into_market_at)
                    );

    debug!("Get stocks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<GetStocksModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error when getting stocks: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}


/////////////

#[derive(Queryable, Serialize)]
pub struct GetNewStocksModel {
    pub id: i32,
    pub issuer_name: String,
    pub name: String,
    pub into_market: bool,
    pub into_market_at: Option<chrono::NaiveDateTime>,
    pub offer_circ: i64,
    pub offer_price: i32,
    pub offer_unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime
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
        move |res: Result<Vec<GetNewStocksModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_ipo_stocks_query(paging: PagingModel, pool: web::Data<Pool>) -> Result<Vec<GetNewStocksModel>, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = stocks.inner_join(users).inner_join(new_stocks).filter(
                    into_market.eq(false)
                )
                    .order(crate::schema::new_stocks::dsl::created_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (crate::schema::stocks::dsl::id, crate::schema::users::dsl::name, crate::schema::stocks::dsl::name, into_market, into_market_at, offer_circ, offer_price, offer_unfulfilled, crate::schema::new_stocks::dsl::created_at)
                    );

    debug!("Get ipo stocks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<GetNewStocksModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error when getting stocks: {}", db_err);
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
        move |res: Result<Vec<Stocks>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_my_stocks_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<Stocks>, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = stocks.filter(
                    into_market.eq(true).and(
                        issuer_id.eq(user.id)
                    )
                )
                    .order(into_market_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?);

    debug!("Get my stocks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<Stocks>(conn)
        .map_err(|db_err| {
            debug!("Database query error when getting stocks: {}", db_err);
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
        move |res: Result<Vec<GetNewStocksModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_my_ipo_stocks_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<GetNewStocksModel>, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = stocks.inner_join(users).inner_join(new_stocks).filter(
                    into_market.eq(false).and(
                        issuer_id.eq(user.id)   
                    )
                )
                    .order(crate::schema::new_stocks::dsl::created_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (crate::schema::stocks::dsl::id, crate::schema::users::dsl::name, crate::schema::stocks::dsl::name, into_market, into_market_at, offer_circ, offer_price, offer_unfulfilled, crate::schema::new_stocks::dsl::created_at)
                    );

    debug!("Get my ipo stocks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<GetNewStocksModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error when getting stocks: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}


//////////////////

pub fn get_stock(
    stock_id: web::Path<u32>,
    _: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_id = stock_id.into_inner();
   
    web::block(
        move || {
            get_stock_query(stock_id, pool)
        }
    ).then(
        move |res: Result<GetNewStocksModel, BlockingError<EngineError>>|
            match res {
                Ok(stock) => Ok(HttpResponse::Ok().json(stock)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_stock_query(stock_id: u32, pool: web::Data<Pool>) -> Result<GetNewStocksModel, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    let stock_id = i32::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 32 字节有符号整数：{}。", try_err)))?;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = stocks.inner_join(users).inner_join(new_stocks).filter(
                    crate::schema::stocks::dsl::id.eq(stock_id)
                )
                .select(
                    (crate::schema::stocks::dsl::id, crate::schema::users::dsl::name, crate::schema::stocks::dsl::name, into_market, into_market_at, offer_circ, offer_price, offer_unfulfilled, crate::schema::new_stocks::dsl::created_at)
                );

    debug!("Get stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_result::<GetNewStocksModel>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::BadRequest(format!("没有这只股票。")))
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
        move |res: Result<GetNewStocksModel, BlockingError<EngineError>>|
            match res {
                Ok(stock) => Ok(HttpResponse::Ok().json(stock)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_stock_by_name_query(stock_name: String, pool: web::Data<Pool>) -> Result<GetNewStocksModel, EngineError> {
    use crate::schema::new_stocks::dsl::*;
    use crate::schema::stocks::dsl::*;
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = stocks.inner_join(users).inner_join(new_stocks).filter(
                    crate::schema::stocks::dsl::name.eq(stock_name)
                )
                .select(
                    (crate::schema::stocks::dsl::id, crate::schema::users::dsl::name, crate::schema::stocks::dsl::name, into_market, into_market_at, offer_circ, offer_price, offer_unfulfilled, crate::schema::new_stocks::dsl::created_at)
                );

    debug!("Get stock by name SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_result::<GetNewStocksModel>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::BadRequest(format!("没有这只股票。")))
}