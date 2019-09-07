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
    web::scope("/orders")
        .service(
            web::resource("/")  // Scope 会自动加尾 /，所以 /orders 无法匹配
                .route(web::post().to_async(new_order))   // 创建委托
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/my/asks/")
                .route(web::get().to_async(get_my_asks))     // 查询自己的买委托
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/my/bids/")
                .route(web::get().to_async(get_my_bids))     // 查询自己的卖委托
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/asks/{id}")
                .route(web::get().to_async(get_ask))      // 获取委托
                .route(web::delete().to_async(revoke_ask))      // 撤销委托
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/bids/{id}")
                .route(web::get().to_async(get_bid))      // 获取委托
                .route(web::delete().to_async(revoke_bid))      // 撤销委托
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
}

#[derive(Debug, Deserialize, Clone)]
pub enum AskOrBid {
    Ask,
    Bid,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OrderModel {
    pub entype: AskOrBid,
    pub stock_id: i64,
    pub price: i32,
    pub volume: i64,
}

#[derive(Queryable, Insertable)]
#[table_name="user_ask_orders"]
pub struct AskOrderModel {
    pub user_id: i64,
    pub stock_id: i64,
    pub price: i32,
    pub volume: i64,
    pub unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime
}

trait AskOrBidOrderModel {
    fn from_order_model_and_user(model: &OrderModel, user: &RememberUserModel) -> Self
        where Self: Sized;
}

impl AskOrBidOrderModel for AskOrderModel {
    fn from_order_model_and_user(model: &OrderModel, user: &RememberUserModel) -> AskOrderModel {
        AskOrderModel {
            user_id: user.id,
            stock_id: model.stock_id,
            price: model.price,
            volume: model.volume,
            unfulfilled: model.volume,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc()
        }
    }
}

#[derive(Queryable, Insertable)]
#[table_name="user_bid_orders"]
pub struct BidOrderModel {
    pub user_id: i64,
    pub stock_id: i64,
    pub price: i32,
    pub volume: i64,
    pub unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime
}

impl AskOrBidOrderModel for BidOrderModel {
    fn from_order_model_and_user(model: &OrderModel, user: &RememberUserModel) -> BidOrderModel {
        BidOrderModel {
            user_id: user.id,
            stock_id: model.stock_id,
            price: model.price,
            volume: model.volume,
            unfulfilled: model.volume,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc()
        }
    }
}

///////////////
#[derive(Queryable, Insertable, Debug)]
#[table_name="deals"]
pub struct NewDeal {
    pub buy_user_id: i64,
    pub sell_user_id: Option<i64>,
    pub stock_id: i64,
    pub price: i32,
    pub amount: i64,
    pub created_at: chrono::NaiveDateTime
}

#[derive(Queryable, Insertable, AsChangeset, Identifiable)]
#[primary_key(user_id, stock_id)]
#[table_name="user_hold_stock"]
pub struct UserStockRel {
    pub user_id: i64,
    pub stock_id: i64,
    pub hold: i64,
    pub updated_at: chrono::NaiveDateTime
}

#[derive(Serialize, Debug)]
pub struct OrderResult {
    pub succeed: bool,
    pub message: Option<String>,
    pub error: Option<String>,
    pub deal_amount: Option<i64>,
    pub lack: Option<i64>
}

impl std::fmt::Display for OrderResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(succeed:{}, deal_amount:{}, lack:{})", self.succeed, self.deal_amount.unwrap_or(0), self.lack.unwrap_or(0))
    }
}

pub fn new_order(
    order: web::Json<OrderModel>,
    curr_user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
   
    web::block(
        move || {
            new_order_query(order.into_inner(), curr_user, pool)
        }
    ).then(
        move |res: Result<i64, BlockingError<EngineError>>|
            match res {
                Ok(deal_num) => Ok(HttpResponse::Ok().json(
                    OrderResult {
                        succeed: true,
                        message: None,
                        error: None,
                        deal_amount: Some(deal_num),
                        lack: None
                    }
                )),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn new_order_query(order: OrderModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<i64, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    conn.transaction(|| {
        // 保证原子性
        // 检查股票是否上市
        let query_stock = stkdsl::stocks
                            .find(order.stock_id)
                            .filter(
                                stkdsl::into_market.eq(true)
                            )
                            .select(stkdsl::id);

        debug!("New order query_stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_stock));

        query_stock.get_result::<i64>(conn)
            .optional()
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库查询错误：{}", db_err))
            })?
            .ok_or_else(|| EngineError::BadRequest(format!("该股票还未上市！")))?;

        // 如果是买单，扣钱；如果是卖单，扣股票

        match order.entype {
            AskOrBid::Ask => {
                let query = diesel::update(usrdsl::users.find(user.id))
                                .set(usrdsl::balance.eq(usrdsl::balance - order.price as i64 * order.volume));

                debug!("New freeze balance query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

                let user_after = query.get_result::<User>(conn)
                    .map_err(|db_err| {
                        debug!("Database query error: {}", db_err);
                        EngineError::InternalError(format!("数据库更新余额错误：{}", db_err))
                    })?;

                if user_after.balance < 0 {
                    let err_msg = format!("账户余额不足，你还需要 {} 元来申请这笔委托。", (-user_after.balance) as f32 / 100.);
                    return Err(EngineError::Insufficient(
                        OrderResult {
                            succeed: false,
                            message: Some(err_msg.clone()),
                            error: Some(err_msg),
                            deal_amount: None,
                            lack: Some(-user_after.balance)
                        }
                    ));
                }
            },
            AskOrBid::Bid => {
                let query = diesel::update(reldsl::user_hold_stock.find(
                                (user.id, order.stock_id)
                            ))
                            .set((
                                reldsl::hold.eq(reldsl::hold - order.volume),
                                reldsl::updated_at.eq(chrono::Utc::now().naive_utc())
                            ));

                debug!("New freeze stock query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

                let rel_after = query.get_result::<UserStockRel>(conn)
                    .optional()
                    .map_err(|db_err| {
                        debug!("Database query error: {}", db_err);
                        EngineError::InternalError(format!("数据库更新股票持有量错误：{}", db_err))
                    })?
                    .ok_or_else(|| {
                        let err_msg = format!("股票持有量不足，你当前并未持有该股票。");
                        EngineError::Insufficient(
                            OrderResult {
                                succeed: false,
                                message: Some(err_msg.clone()),
                                error: Some(err_msg),
                                deal_amount: None,
                                lack: Some(-order.volume)
                            }
                        )
                    })?;

                if rel_after.hold < 0 {
                    let err_msg = format!("股票持有量不足，你还需要 {} 股来申请这笔委托。", -rel_after.hold);
                    return Err(EngineError::Insufficient(
                        OrderResult {
                            succeed: false,
                            message: Some(err_msg.clone()),
                            error: Some(err_msg),
                            deal_amount: None,
                            lack: Some(-rel_after.hold)
                        }
                    ));
                }
            }
        }

        // 创建委托单
        let mut new_ask: Option<AskOrder> = None;
        let mut new_bid: Option<BidOrder> = None;
        match order.entype {
            AskOrBid::Ask => {
                let query = diesel::insert_into(askdsl::user_ask_orders)
                    .values(AskOrderModel::from_order_model_and_user(&order, &user));

                debug!("New order query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

                new_ask = Some(query.get_result::<AskOrder>(conn)
                    .map_err(|db_err| {
                        debug!("Database query error: {}", db_err);
                        EngineError::InternalError(format!("数据库插入委托错误：{}", db_err))
                    })?);

                

            },
            AskOrBid::Bid => {
                let query = diesel::insert_into(biddsl::user_bid_orders)
                    .values(BidOrderModel::from_order_model_and_user(&order, &user));

                debug!("New order query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

                new_bid = Some(query.get_result::<BidOrder>(conn)
                    .map_err(|db_err| {
                        debug!("Database query error: {}", db_err);
                        EngineError::InternalError(format!("数据库插入委托错误：{}", db_err))
                    })?);
            }
        }

        // 第三步：撮合
        let deal_num = match order.entype {
            AskOrBid::Ask => {
                let mut new_ask = new_ask.ok_or_else(|| EngineError::InternalError(format!("服务端逻辑错误。")))?;
                let query = biddsl::user_bid_orders.filter(
                                biddsl::stock_id.eq(order.stock_id).and(
                                    biddsl::unfulfilled.ne(0)
                                ).and(
                                    biddsl::price.le(order.price)
                                )
                            )
                                .order_by(
                                    biddsl::price.asc()
                                )
                                .then_order_by(
                                    biddsl::created_at.asc()   
                                );
                            

                debug!("Ask matching query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

                let mut bids = query.get_results::<BidOrder>(conn)
                    .map_err(|db_err| {
                        debug!("Database query error: {}", db_err);
                        EngineError::InternalError(format!("数据库查询卖出委托错误：{}", db_err))
                    })?;

                let mut deal_num = 0;

                for bid in &mut bids {
                    let deal_amount = std::cmp::min(bid.unfulfilled, new_ask.unfulfilled);
                    new_ask.unfulfilled -= deal_amount;
                    new_ask.updated_at = chrono::Utc::now().naive_utc();
                    bid.unfulfilled -= deal_amount;
                    bid.updated_at = chrono::Utc::now().naive_utc();
                    let deal = NewDeal {
                        buy_user_id: new_ask.user_id,
                        sell_user_id: Some(bid.user_id),
                        stock_id: new_ask.stock_id,
                        price: bid.price,
                        amount: deal_amount,
                        created_at: chrono::Utc::now().naive_utc()
                    };
                    debug!("Deal: {:?}", deal);
                    deal_num += deal_amount;
                    let give_seller_cash = deal_amount * (deal.price as i64);
                    let giveback_buyer_cash = deal_amount * ((new_ask.price - deal.price) as i64);
                    
                    new_ask.save_changes::<AskOrder>(conn).map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设新买委托错误：{}", db_err))
                        })?;
                    bid.save_changes::<BidOrder>(conn).map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设旧卖委托错误：{}", db_err))
                        })?;
                    diesel::update(usrdsl::users.find(deal.buy_user_id))
                        .set(usrdsl::balance.eq(usrdsl::balance + giveback_buyer_cash))
                        .execute(conn)
                        .map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设买家余额错误：{}", db_err))
                        })?;
                    diesel::update(usrdsl::users.find(bid.user_id))
                        .set(usrdsl::balance.eq(usrdsl::balance + give_seller_cash))
                        .execute(conn)
                        .map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设卖家余额错误：{}", db_err))
                        })?;
                    diesel::insert_into(dldsl::deals).values(&deal)
                        .execute(conn)
                        .map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库插入交易错误：{}", db_err))
                        })?;
                    diesel::insert_into(reldsl::user_hold_stock)
                        .values(
                            UserStockRel {
                                user_id: deal.buy_user_id,
                                stock_id: deal.stock_id,
                                hold: deal.amount,
                                updated_at: chrono::Utc::now().naive_utc()
                            }
                        )
                        .on_conflict((reldsl::user_id, reldsl::stock_id))
                        .do_update()
                        .set((
                            reldsl::hold.eq(reldsl::hold + deal.amount),
                            reldsl::updated_at.eq(chrono::Utc::now().naive_utc())
                        ))
                        .execute(conn)
                        .map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设买家股票数量错误：{}", db_err))
                        })?;

                    if new_ask.unfulfilled == 0 {
                        break;
                    }
                }

                deal_num
            },
            AskOrBid::Bid => {
                let mut new_bid = new_bid.ok_or_else(|| EngineError::InternalError(format!("服务端逻辑错误。")))?;
                let query = askdsl::user_ask_orders.filter(
                                askdsl::stock_id.eq(order.stock_id).and(
                                    askdsl::unfulfilled.ne(0)
                                ).and(
                                    askdsl::price.ge(order.price)
                                )
                            )
                                .order_by(
                                    askdsl::price.desc()
                                )
                                .then_order_by(
                                    askdsl::created_at.asc()   
                                );
                            

                debug!("Ask matching query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

                let mut asks = query.get_results::<AskOrder>(conn)
                    .map_err(|db_err| {
                        debug!("Database query error: {}", db_err);
                        EngineError::InternalError(format!("数据库查询查找委托错误：{}", db_err))
                    })?;

                let mut deal_num = 0;

                for ask in &mut asks {
                    let deal_amount = std::cmp::min(ask.unfulfilled, new_bid.unfulfilled);
                    new_bid.unfulfilled -= deal_amount;
                    new_bid.updated_at = chrono::Utc::now().naive_utc();
                    ask.unfulfilled -= deal_amount;
                    ask.updated_at = chrono::Utc::now().naive_utc();
                    let deal = NewDeal {
                        buy_user_id: ask.user_id,
                        sell_user_id: Some(new_bid.user_id),
                        stock_id: new_bid.stock_id,
                        price: ask.price,
                        amount: deal_amount,
                        created_at: chrono::Utc::now().naive_utc()
                    };
                    debug!("Deal: {:?}", deal);
                    deal_num += deal_amount;
                    let give_seller_cash = deal_amount * (deal.price as i64);
                    // let giveback_buyer_cash = 0;
                    
                    new_bid.save_changes::<BidOrder>(conn).map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设新卖委托错误：{}", db_err))
                        })?;
                    ask.save_changes::<AskOrder>(conn).map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设旧买委托错误：{}", db_err))
                        })?;
                    diesel::update(usrdsl::users.find(new_bid.user_id))
                        .set(usrdsl::balance.eq(usrdsl::balance + give_seller_cash))
                        .execute(conn)
                        .map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设卖家余额错误：{}", db_err))
                        })?;
                    diesel::insert_into(dldsl::deals).values(&deal)
                        .execute(conn)
                        .map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库插入交易错误：{}", db_err))
                        })?;
                    diesel::insert_into(reldsl::user_hold_stock)
                        .values(
                            UserStockRel {
                                user_id: deal.buy_user_id,
                                stock_id: deal.stock_id,
                                hold: deal.amount,
                                updated_at: chrono::Utc::now().naive_utc()
                            }
                        )
                        .on_conflict((reldsl::user_id, reldsl::stock_id))
                        .do_update()
                        .set((
                            reldsl::hold.eq(reldsl::hold + deal.amount),
                            reldsl::updated_at.eq(chrono::Utc::now().naive_utc())
                        ))
                        .execute(conn)
                        .map_err(|db_err| {
                            debug!("Database query error: {}", db_err);
                            EngineError::InternalError(format!("数据库重设买家股票数量错误：{}", db_err))
                        })?;

                    if new_bid.unfulfilled == 0 {
                        break;
                    }
                }

                deal_num
            }
        };

        Ok(deal_num)
    })
}



/////////////
#[derive(Serialize, Debug, Queryable)]
pub struct ReturnOrderModel {
    pub id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub stock_id: i64,
    pub stock_name: String,
    pub price: i32,
    pub volume: i64,
    pub unfulfilled: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime
}

pub fn get_my_asks(
    paging: web::Query<PagingModel>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_my_asks_query(paging, user, pool)
        }
    ).then(
        move |res: Result<Vec<ReturnOrderModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_my_asks_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<ReturnOrderModel>, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = askdsl::user_ask_orders
                    .inner_join(usrdsl::users)
                    .inner_join(stkdsl::stocks)
                    .filter(
                        askdsl::user_id.eq(user.id)
                    )
                    .order(askdsl::created_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (
                            askdsl::id,
                            usrdsl::id,
                            usrdsl::name,
                            stkdsl::id,
                            stkdsl::name,
                            askdsl::price,
                            askdsl::volume,
                            askdsl::unfulfilled,
                            askdsl::created_at,
                            askdsl::updated_at
                        )
                    );

    debug!("Get my asks SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<ReturnOrderModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}

//////////////////
pub fn get_my_bids(
    paging: web::Query<PagingModel>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_my_bids_query(paging, user, pool)
        }
    ).then(
        move |res: Result<Vec<ReturnOrderModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_my_bids_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<ReturnOrderModel>, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = biddsl::user_bid_orders
                    .inner_join(usrdsl::users)
                    .inner_join(stkdsl::stocks)
                    .filter(
                        biddsl::user_id.eq(user.id)
                    )
                    .order(biddsl::created_at.desc())
                    .offset(paging.offset.unwrap_or(0).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .limit(paging.limit.unwrap_or(10).try_into().map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .select(
                        (
                            biddsl::id,
                            usrdsl::id,
                            usrdsl::name,
                            stkdsl::id,
                            stkdsl::name,
                            biddsl::price,
                            biddsl::volume,
                            biddsl::unfulfilled,
                            biddsl::created_at,
                            biddsl::updated_at
                        )
                    );

    debug!("Get my bids SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_results::<ReturnOrderModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}


//////////////////
pub fn get_ask(
    ask_id: web::Path<u64>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let ask_id = ask_id.into_inner();
   
    web::block(
        move || {
            get_ask_query(ask_id, user, pool)
        }
    ).then(
        move |res: Result<ReturnOrderModel, BlockingError<EngineError>>|
            match res {
                Ok(order) => Ok(HttpResponse::Ok().json(order)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_ask_query(ask_id: u64, _: RememberUserModel, pool: web::Data<Pool>) -> Result<ReturnOrderModel, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let ask_id = i64::try_from(ask_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    let query = askdsl::user_ask_orders
                    .inner_join(usrdsl::users)
                    .inner_join(stkdsl::stocks)
                    .filter(
                        askdsl::id.eq(ask_id)
                    )
                    .select(
                        (
                            askdsl::id,
                            usrdsl::id,
                            usrdsl::name,
                            stkdsl::id,
                            stkdsl::name,
                            askdsl::price,
                            askdsl::volume,
                            askdsl::unfulfilled,
                            askdsl::created_at,
                            askdsl::updated_at
                        )
                    );

    debug!("Get ask SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_result::<ReturnOrderModel>(conn)
        .optional()
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?
        .ok_or_else(|| {
            EngineError::NotFound(format!("未找到请求的委托。"))
        })
}


//////////////////
pub fn get_bid(
    bid_id: web::Path<u64>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let bid_id = bid_id.into_inner();
   
    web::block(
        move || {
            get_bid_query(bid_id, user, pool)
        }
    ).then(
        move |res: Result<ReturnOrderModel, BlockingError<EngineError>>|
            match res {
                Ok(order) => Ok(HttpResponse::Ok().json(order)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_bid_query(bid_id: u64, _: RememberUserModel, pool: web::Data<Pool>) -> Result<ReturnOrderModel, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let bid_id = i64::try_from(bid_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    let query = biddsl::user_bid_orders
                    .inner_join(usrdsl::users)
                    .inner_join(stkdsl::stocks)
                    .filter(
                        biddsl::id.eq(bid_id)
                    )
                    .select(
                        (
                            biddsl::id,
                            usrdsl::id,
                            usrdsl::name,
                            stkdsl::id,
                            stkdsl::name,
                            biddsl::price,
                            biddsl::volume,
                            biddsl::unfulfilled,
                            biddsl::created_at,
                            biddsl::updated_at
                        )
                    );

    debug!("Get bid SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_result::<ReturnOrderModel>(conn)
        .optional()
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })?
        .ok_or_else(|| {
            EngineError::NotFound(format!("未找到请求的委托。"))
        })
}


//////////////////
pub fn revoke_ask(
    ask_id: web::Path<u64>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let ask_id = ask_id.into_inner();
   
    web::block(
        move || {
            revoke_ask_query(ask_id, user, pool)
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

fn revoke_ask_query(ask_id: u64, user: RememberUserModel, pool: web::Data<Pool>) -> Result<(), EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let ask_id = i64::try_from(ask_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    use diesel::pg::expression::array_comparison::AsArrayExpression;

    conn.transaction(|| {
        // 保证原子性

        // 返钱

        let (ask_unful, ask_price): (i64, i32)
            = askdsl::user_ask_orders.filter(
                    askdsl::id.eq(ask_id).and(
                        askdsl::user_id.eq(user.id)
                    )
                ).limit(1)
                .select(
                    (askdsl::unfulfilled, askdsl::price)
                )
                .get_result(conn)
                .optional()
                .map_err(|db_err| {
                    debug!("Database query error: {}", db_err);
                    EngineError::InternalError(format!("数据库查询错误：{}", db_err))
                })?
                .ok_or_else(|| {
                    EngineError::NotFound(format!("未找到请求的委托。"))
                })?;

        let query = diesel::update(usrdsl::users.find(user.id))
                        .set(usrdsl::balance.eq(usrdsl::balance + ask_unful * ask_price as i64));

        debug!("New refund balance query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

        let affected_rows = query.execute(conn)
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库更新余额错误：{}", db_err))
            })?;

        match affected_rows {
            1 => Ok(()),
            _ => Err(EngineError::InternalError(format!("数据库更新余额，影响行数非 1：{}", affected_rows)))
        }?;

        // 删除委托单
        let query = diesel::delete(askdsl::user_ask_orders.find(ask_id));

        debug!("New delete query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

        let affected_rows = query.execute(conn)
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库删除委托错误：{}", db_err))
            })?;

        match affected_rows {
            1 => Ok(()),
            _ => Err(EngineError::InternalError(format!("数据库删除委托，影响行数非 1：{}", affected_rows)))
        }?;

        Ok(())
    })
}


//////////////////
pub fn revoke_bid(
    bid_id: web::Path<u64>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let bid_id = bid_id.into_inner();
   
    web::block(
        move || {
            revoke_bid_query(bid_id, user, pool)
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

fn revoke_bid_query(bid_id: u64, user: RememberUserModel, pool: web::Data<Pool>) -> Result<(), EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let bid_id = i64::try_from(bid_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    use diesel::pg::expression::array_comparison::AsArrayExpression;

    conn.transaction(|| {
        // 保证原子性

        // 返还股票
        let (bid_unful, stock_id): (i64, i64)
            = biddsl::user_bid_orders.filter(
                    biddsl::id.eq(bid_id).and(
                        biddsl::user_id.eq(user.id)
                    )
                ).limit(1)
                .select(
                    (biddsl::unfulfilled, biddsl::stock_id)
                )
                .get_result(conn)
                .optional()
                .map_err(|db_err| {
                    debug!("Database query error: {}", db_err);
                    EngineError::InternalError(format!("数据库查询错误：{}", db_err))
                })?
                .ok_or_else(|| {
                    EngineError::NotFound(format!("未找到请求的委托。"))
                })?;

        let query = diesel::update(reldsl::user_hold_stock.find((user.id, stock_id)))
                        .set((
                            reldsl::hold.eq(reldsl::hold + bid_unful),
                            reldsl::updated_at.eq(chrono::Utc::now().naive_utc())
                        ));

        debug!("New refund stock query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

        let affected_rows = query.execute(conn)
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库更新股票持有量错误：{}", db_err))
            })?;

        match affected_rows {
            1 => Ok(()),
            _ => Err(EngineError::InternalError(format!("数据库更新股票持有量，影响行数非 1：{}", affected_rows)))
        }?;

        // 删除委托单
        let query = diesel::delete(biddsl::user_bid_orders.find(bid_id));

        debug!("New delete query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

        let affected_rows = query.execute(conn)
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库删除委托错误：{}", db_err))
            })?;

        match affected_rows {
            1 => Ok(()),
            _ => Err(EngineError::InternalError(format!("数据库删除委托，影响行数非 1：{}", affected_rows)))
        }?;

        Ok(())
    })
}


/////////////////
#[derive(Debug, Deserialize, Clone)]
pub struct IPOBuyModel {
    pub amount: u64
}

pub fn ipo_buy(
    stock_id: web::Path<u64>,
    ipobuy: web::Json<IPOBuyModel>,
    curr_user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
   
    web::block(
        move || {
            ipo_buy_query(stock_id.into_inner(), ipobuy.into_inner(), curr_user, pool)
        }
    ).then(
        move |res: Result<i64, BlockingError<EngineError>>|
            match res {
                Ok(deal_num) => Ok(HttpResponse::Ok().json(
                    OrderResult {
                        succeed: true,
                        message: None,
                        error: None,
                        deal_amount: Some(deal_num),
                        lack: None
                    }
                )),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn ipo_buy_query(stock_id: u64, ipobuy: IPOBuyModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<i64, EngineError> {
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

    let amount = i64::try_from(ipobuy.amount).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    conn.transaction(|| {
        // 保证原子性
        // 检查股票是否上市
        let query_stock = stkdsl::stocks
                            .find(stock_id)
                            .filter(
                                stkdsl::into_market.eq(false)
                            )
                            .select(stkdsl::id);

        debug!("New ipobuy query_stock SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_stock));

        query_stock.get_result::<i64>(conn)
            .optional()
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库查询错误：{}", db_err))
            })?
            .ok_or_else(|| EngineError::BadRequest(format!("没有该股票或该股票已经上市，不能再 IPO 买入！")))?;

        // 检查 IPO unfulfulled，扣除
        let target_ipo = newdsl::new_stocks
                            .filter(newdsl::id.eq(stock_id));

        debug!("New ipobuy target_ipo SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&target_ipo));

        let mut new_stock = target_ipo.get_result::<NewStock>(conn)
            .optional()
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库查询错误：{}", db_err))
            })?
            .ok_or_else(|| EngineError::InternalError(format!("该股票没有新股发行信息，请联系管理员维护！")))?;

        let effective_amount = std::cmp::min(new_stock.offer_unfulfilled, amount);

        new_stock.offer_unfulfilled -= effective_amount;
        new_stock.save_changes::<NewStock>(conn).map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库重设 IPO 发行余量错误：{}", db_err))
        })?;

        // 检查钱，扣钱

        let query_charge = diesel::update(usrdsl::users.find(user.id))
                        .set(usrdsl::balance.eq(usrdsl::balance - new_stock.offer_price as i64 * effective_amount));

        debug!("New charge balance query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query_charge));

        let user_after = query_charge.get_result::<User>(conn)
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库更新余额错误：{}", db_err))
            })?;

        if user_after.balance < 0 {
            let err_msg = format!("账户余额不足，你还需要 {} 元来购买剩余数量的新发行股票。", (-user_after.balance) as f32 / 100.);
            return Err(EngineError::Insufficient(
                OrderResult {
                    succeed: false,
                    message: Some(err_msg.clone()),
                    error: Some(err_msg),
                    deal_amount: None,
                    lack: Some(-user_after.balance)
                }
            ));
        }

        // 加交易、加股票

        let deal = NewDeal {
            buy_user_id: user.id,
            sell_user_id: None,
            stock_id: stock_id,
            price: new_stock.offer_price,
            amount: effective_amount,
            created_at: chrono::Utc::now().naive_utc()
        };
        
        diesel::insert_into(dldsl::deals).values(&deal)
            .execute(conn)
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库插入交易错误：{}", db_err))
            })?;

        diesel::insert_into(reldsl::user_hold_stock)
            .values(
                UserStockRel {
                    user_id: deal.buy_user_id,
                    stock_id: deal.stock_id,
                    hold: deal.amount,
                    updated_at: chrono::Utc::now().naive_utc()
                }
            )
            .on_conflict((reldsl::user_id, reldsl::stock_id))
            .do_update()
            .set((
                reldsl::hold.eq(reldsl::hold + deal.amount),
                reldsl::updated_at.eq(chrono::Utc::now().naive_utc())
            ))
            .execute(conn)
            .map_err(|db_err| {
                debug!("Database query error: {}", db_err);
                EngineError::InternalError(format!("数据库重设买家股票数量错误：{}", db_err))
            })?;

        Ok(effective_amount)
    })
}



//////////////////
#[derive(QueryableByName, Serialize, Deserialize)]
pub struct DealModel {
    #[sql_type = "sql_types::BigInt"]
    pub id: i64,
    #[sql_type = "sql_types::BigInt"]
    pub stock_id: i64,
    #[sql_type = "sql_types::Varchar"]
    pub stock_name: String,
    #[sql_type = "sql_types::BigInt"]
    pub buy_user_id: i64,
    #[sql_type = "sql_types::Varchar"]
    pub buy_user_name: String,
    #[sql_type = "sql_types::Nullable<sql_types::BigInt>"]
    pub sell_user_id: Option<i64>,  // 当是 NULL 时，表示是购买发行新股
    #[sql_type = "sql_types::Nullable<sql_types::Varchar>"]
    pub sell_user_name: Option<String>,
    #[sql_type = "sql_types::Int4"]
    pub price: i32,
    #[sql_type = "sql_types::Int8"]
    pub amount: i64,
    #[sql_type = "sql_types::Timestamp"]
    pub created_at: chrono::NaiveDateTime
}


pub fn get_my_deals(
    paging: web::Query<PagingModel>,
    user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
    let paging = paging.into_inner();
   
    web::block(
        move || {
            get_my_deals_query(paging, user, pool)
        }
    ).then(
        move |res: Result<Vec<DealModel>, BlockingError<EngineError>>|
            match res {
                Ok(stocks) => Ok(HttpResponse::Ok().json(stocks)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_my_deals_query(paging: PagingModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<Vec<DealModel>, EngineError> {
    use crate::schema::stocks::dsl as stkdsl;
    use crate::schema::users::dsl as usrdsl;
    use crate::schema::user_hold_stock::dsl as reldsl;
    use crate::schema::deals::dsl as dldsl;
    use crate::schema::user_ask_orders::dsl as askdsl;
    use crate::schema::user_bid_orders::dsl as biddsl;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = diesel::sql_query(include_str!("mydeals.sql"))
                    .bind::<sql_types::BigInt, _>(user.id)
                    .bind::<sql_types::Int8, _>(i64::try_from(paging.offset.unwrap_or(0)).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?)
                    .bind::<sql_types::Int8, _>(i64::try_from(paging.limit.unwrap_or(10)).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 64 字节有符号整数：{}。", try_err)))?);

    debug!("Get my deals SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.load::<DealModel>(conn)
        .map_err(|db_err| {
            debug!("Database query error: {}", db_err);
            EngineError::InternalError(format!("数据库查询错误：{}", db_err))
        })
}
