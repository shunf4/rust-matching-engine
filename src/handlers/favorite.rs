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

pub fn make_scope() -> actix_web::Scope {
    web::scope("/favorites")
        .service(
            web::resource("/")  // Scope 会自动加尾 /，所以 /favorites 无法匹配
                .route(web::get().to_async(get_favorites))   // 获取收藏列表
                .route(web::post().to_async(add_favorite))   // 新增收藏
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/{ids}/is_favorited")
                .route(web::get().to_async(get_is_favorited))     // 删除收藏
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/{id}")
                .route(web::delete().to_async(delete_favorite))     // 删除收藏
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
}


/////////////
/// 
#[derive(Deserialize)]
pub struct AddFavoriteModel {
    pub stock_id: u64
}

pub fn add_favorite(
    input: web::Json<AddFavoriteModel>,
    curr_user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_id = input.into_inner().stock_id;
   
    web::block(
        move || {
            add_favorite_query(stock_id, curr_user, pool)
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

fn add_favorite_query(stock_id: u64, curr_user: RememberUserModel, pool: web::Data<Pool>) -> Result<(), EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::user_fav_stock::dsl as favdsl;
    use crate::schema::users::dsl as usrdsl;

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = diesel::insert_into(favdsl::user_fav_stock)
                            .values((
                                favdsl::user_id.eq(curr_user.id),
                                favdsl::stock_id.eq(stock_id),
                                favdsl::created_at.eq(chrono::Utc::now().naive_utc())
                            ));

    debug!("Addfav stock query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.execute(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库插入收藏错误：{}", db_err))
        })?;

    Ok(())
}


/////////////

pub fn delete_favorite(
    stock_id: web::Path<u64>,
    curr_user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_id = stock_id.into_inner();
   
    web::block(
        move || {
            delete_favorite_query(stock_id, curr_user, pool)
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

fn delete_favorite_query(stock_id: u64, curr_user: RememberUserModel, pool: web::Data<Pool>) -> Result<(), EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::user_fav_stock::dsl as favdsl;
    use crate::schema::users::dsl as usrdsl;

    let stock_id = i64::try_from(stock_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = diesel::delete(favdsl::user_fav_stock.find(
                                (curr_user.id, stock_id)
                            ));

    debug!("Delfav stock query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    let affecting_rows = query.execute(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库插入收藏错误：{}", db_err))
        })?;

    match affecting_rows {
        1 => Ok(()),
        0 => Err(EngineError::NotFound(format!("找不到这支收藏的股票。"))),
        _ => Err(EngineError::InternalError(format!("数据库删除返回值错误：{}", affecting_rows)))
    }
}


/////////////
#[derive(Queryable, Serialize)]
pub struct FavoriteModel {
    pub id: i64,
    pub issuer_name: Option<String>,
    pub name: String,
    pub into_market: bool,
    pub into_market_at: Option<chrono::NaiveDateTime>,
    pub offer_circ: Option<i64>,
    pub offer_price: Option<i32>,
    pub offer_unfulfilled: Option<i64>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub favorited_at: chrono::NaiveDateTime
}

pub fn get_favorites(
    paging: web::Query<PagingModel>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_favorites_query(paging, user, pool)
        }
    ).then(
        move |res: Result<Vec<FavoriteModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_favorites_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<FavoriteModel>, EngineError> {
    use crate::schema::new_stocks::dsl as newdsl;
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::user_fav_stock::dsl as favdsl;
    use crate::schema::users::dsl as usrdsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = favdsl::user_fav_stock.inner_join(stkdsl::stocks.left_join(newdsl::new_stocks.inner_join(usrdsl::users))).filter(
                    favdsl::user_id.eq(user.id)
                )
                .select(
                    (crate::schema::stocks::dsl::id, crate::schema::users::dsl::name.nullable(), crate::schema::stocks::dsl::name, stkdsl::into_market, stkdsl::into_market_at, newdsl::offer_circ.nullable(), newdsl::offer_price.nullable(), newdsl::offer_unfulfilled.nullable(), crate::schema::new_stocks::dsl::created_at.nullable(), favdsl::created_at)
                )
                .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?);

    match paging.order.unwrap_or(super::PagingOrder::Latest) {
        super::PagingOrder::Alphabetical => {
            let query = query.order_by(stkdsl::name.asc());
            debug!("Get fav SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));
            query.get_results::<FavoriteModel>(conn)
                .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))
        }

        super::PagingOrder::Latest => {
            let query = query.order_by(favdsl::created_at.desc());
            debug!("Get fav SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));
            query.get_results::<FavoriteModel>(conn)
                .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))
        }
    }
}

/////////////
#[derive(QueryableByName, Serialize)]
pub struct IsFavoritedModel {
    #[sql_type = "sql_types::Bool"]
    pub is_favorited: bool
}

pub fn get_is_favorited(
    stock_ids: web::Path<String>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let stock_ids = stock_ids.into_inner();
   
    web::block(
        move || {
            let stock_ids: Vec<u64> = serde_json::from_str(&stock_ids[..]).map_err(|json_err| EngineError::BadRequest(format!("解析股票 ID 列表错误：{}", json_err)))?;
            get_is_favorited_query(stock_ids, user, pool)
        }
    ).then(
        move |res: Result<Vec<IsFavoritedModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_is_favorited_query(stock_ids: Vec<u64>, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<IsFavoritedModel>, EngineError> {
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

    let query = diesel::sql_query(include_str!("isfavorited.sql"))
                    .bind::<sql_types::Array<sql_types::BigInt>, _>(&stock_ids)
                    .bind::<sql_types::BigInt, _>(&user.id);

    debug!("Get is_favorited SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.load::<IsFavoritedModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}