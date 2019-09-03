use actix_web::{
    web, Error, HttpRequest, HttpResponse, FromRequest
};
use actix_web::error::BlockingError;
use actix_identity::Identity;
use crate::models::User;

use std::convert::TryFrom;
use std::convert::TryInto;

use futures::Future;
use crate::errors::EngineError;

use crate::common::Pool;
use diesel::PgConnection;
use diesel::prelude::*;

use std::str::FromStr;

use super::users::{RememberUserModel};



#[derive(Debug, Deserialize, Clone)]
pub struct RechargeModel {
    pub cash: u64,
}

pub fn recharge(
    recharge: web::Json<RechargeModel>,
    curr_user: RememberUserModel,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {
   
    web::block(
        move || {
            recharge_query(recharge.into_inner(), curr_user, pool)
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

fn recharge_query(recharge: RechargeModel, user: RememberUserModel, pool: web::Data<Pool>) -> Result<(), EngineError> {
    use crate::schema::users::dsl::*;

    let recharge_cash = i64::try_from(recharge.cash).map_err(|try_err| EngineError::InternalError(format!("输入的整数无法安全转为 64 字节有符号整数：{}。", try_err)))?;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = diesel::update(&user)
                    .set(balance.eq(balance + recharge_cash));

    debug!("Recharege SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_result::<User>(conn)
        .optional()
        .map_err(|db_err| {
            debug!("Database update error when recharging: {}", db_err);
            EngineError::InternalError(format!("数据库更新用户余额错误：{}", db_err))
        })?
        .ok_or_else(|| EngineError::InternalError(format!("未找到当前登录的用户")))?;
    
    Ok(())
}
