use actix_web::{
    web, Error, HttpRequest, HttpResponse
};
use actix_web::error::BlockingError;
use actix_identity::Identity;
use crate::models::User;

use futures::Future;
use crate::errors::EngineError;

use crate::common::Pool;
use std::sync::Arc;
use diesel::PgConnection;
use diesel::prelude::*;


pub fn make_user_scope() -> actix_web::Scope {
    web::scope("/users")
        .service(
            web::resource("/")  // Scope 会自动加尾 /，所以 /users 无法匹配
                .route(web::get().to(|| Err::<(), EngineError>(EngineError::BadRequest(format!("错误：您没有权限获取用户列表。")))))
                .route(web::post().to(|| Err::<(), EngineError>(EngineError::BadRequest(format!("错误：您没有权限编辑用户列表。")))))
                .route(web::put().to_async(register))   // 注册
                .to(|| HttpResponse::MethodNotAllowed())
        )
        .service(
            web::resource("/{id}")
                .route(web::get().to_async(get_user))      // 获取用户
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
        .service(
            web::resource("/by-name/{name}")
                .route(web::get().to_async(get_user_by_name))      // 获取用户
                .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
        )
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegisterModel {
    pub name: String,
    pub password: String
}

#[derive(Debug, Deserialize)]
pub struct RegisterModelWithoutPassword {
    pub name: String,
}

impl From<RegisterModel> for RegisterModelWithoutPassword {
    fn from(r: RegisterModel) -> Self {
        RegisterModelWithoutPassword {
            name: r.name
        }
    }
}

use crate::schema::*;
#[derive(Debug, Deserialize, Insertable)]
#[table_name="users"]
pub struct RegisteringUserModel {
    pub password_hashed: String,
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
    pub balance: i32
}

impl Into<RegisteringUserModel> for RegisterModel {
    fn into(self: Self) -> RegisteringUserModel {
        RegisteringUserModel {
            password_hashed: crate::hash::hash_password(&self.password[..]),
            name: self.name,
            created_at: chrono::Utc::now().naive_utc(),
            balance: 0
        }
    }
}

///////////////


pub fn register(
    register: web::Json<RegisterModel>,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {

    // 这里 register.clone() 会自动对 register 解一次引用，但实际我不想解引用
    let reg_model : RegisterModel = register.into_inner();
    let copied = reg_model.clone();
    let arc_pool = Arc::new(pool);
    let arc_pool_1 = arc_pool.clone();
    let arc_pool_2 = arc_pool.clone();

    web::block(
        move || {
            register_check_duplicate_query(copied.into(), arc_pool_1.clone())
        }
    ).and_then(
        move |_| {
            register_insert_query(reg_model, arc_pool_2.clone()).map_err(|eng_err| BlockingError::Error(eng_err))
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

fn register_check_duplicate_query(reg_data: RegisterModelWithoutPassword, pool: Arc<web::Data<Pool>>) -> Result<(), EngineError> {
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = users.filter(
            name.eq(reg_data.name)
        )
            .limit(1)
            .select(name);
    debug!("Check duplicate SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    if let Some(_) = 
        query
            .get_result::<String>(conn)
            .optional()
            .map_err(|db_err| EngineError::InternalError(format!("数据库插入错误：{}", db_err)))? {
        return Err(EngineError::BadRequest("注册失败，已有重名用户。".to_owned()));
    }

    Ok(())
}

fn register_insert_query(reg_data: RegisterModel, pool: Arc<web::Data<Pool>>) -> Result<(), EngineError> {
    use crate::schema::users::dsl::*;

    // 取出数据库连接
    let conn: &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query = diesel::dsl::insert_into(users)
        .values::<RegisteringUserModel>(
            reg_data.into()
        );

    debug!("Registering SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));

    query.get_result::<User>(conn)
        .map_err(|db_err| {
            debug!("Database insert error when registering: {}", db_err);
            EngineError::InternalError(format!("数据库插入错误：{}", db_err))
        }).map(|_| ())
}

//////////////

#[derive(Debug, Deserialize, Serialize, Queryable)]
pub struct FetchUserModel {
    pub id: i32,
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
    pub balance: i32
}

pub fn get_user(
    user_id: web::Path<u32>,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {

    web::block(
        move || {
            get_user_query(user_id.into_inner(), pool)
        }
    ).then(
        move |res: Result<FetchUserModel, BlockingError<EngineError>>|
            match res {
                Ok(fetch_user_model) => Ok(HttpResponse::Ok().json(fetch_user_model)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_user_query(user_id: u32, pool: web::Data<Pool>) -> Result<FetchUserModel, EngineError> {
    use crate::schema::users::dsl::*;
    use std::convert::TryFrom;

    let user_id = i32::try_from(user_id).map_err(|try_err| EngineError::InternalError(format!("输入的整数太大，无法安全转为 32 字节有符号整数：{}。", try_err)))?;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query =
        users
            .filter(
                id.eq(user_id)
            )
            .select(
                (id, name, created_at, balance)
            );

    debug!("User query SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));
            
    query
        .get_result::<FetchUserModel>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::BadRequest(format!("查询错误，没有该用户。")))
}

//////////////////

pub fn get_user_by_name(
    user_name: web::Path<String>,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {

    web::block(
        move || {
            get_user_by_name_query(user_name.into_inner(), pool)
        }
    ).then(
        move |res: Result<FetchUserModel, BlockingError<EngineError>>|
            match res {
                Ok(fetch_user_model) => Ok(HttpResponse::Ok().json(fetch_user_model)),
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn get_user_by_name_query(user_name: String, pool: web::Data<Pool>) -> Result<FetchUserModel, EngineError> {
    use crate::schema::users::dsl::*;
    use std::convert::TryFrom;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query =
        users
            .filter(
                name.eq(user_name)
            )
            .select(
                (id, name, created_at, balance)
            );

    debug!("User query by name SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));
            
    query
        .get_result::<FetchUserModel>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::BadRequest(format!("查询错误，没有该用户。")))
}


//////////////////

#[derive(Debug, Deserialize, Clone)]
pub struct LoginModel {
    pub name: String,
    pub password: String
}

#[derive(Debug, Clone, Queryable, Serialize)]
pub struct RememberUserModel {
    pub id: i32,
    pub name: String,
}

pub fn login(
    user: web::Json<LoginModel>,
    iden: Identity,
    pool: web::Data<Pool>   // 此处将之前附加到应用的数据库连接取出
) -> impl Future<Item = HttpResponse, Error = EngineError> {

    web::block(
        move || {
            login_query(user.into_inner(), pool)
        }
    ).then(
        move |res: Result<RememberUserModel, BlockingError<EngineError>>|
            match res {
                Ok(remember_user) => {
                    let user_string = serde_json::to_string(&remember_user)
                        .map_err(|json_err| EngineError::InternalError(format!("服务端编码 JSON 错误：{}", json_err)))?;
                    debug!("User login token: {}", user_string);
                    iden.remember(user_string);
                    Ok(HttpResponse::Ok().finish())
                },
                Err(err) => match err {
                    BlockingError::Error(eng_err) => Err(eng_err),
                    BlockingError::Canceled => Err(EngineError::InternalError("不明原因，内部请求被中断。服务端遇到错误。".to_owned()))
                }
            }
    )
}

fn login_query(user: LoginModel, pool: web::Data<Pool>) -> Result<RememberUserModel, EngineError> {
    use crate::schema::users::dsl::*;
    use std::convert::TryFrom;

    // 取出数据库连接
    let conn : &PgConnection = &*(pool.get().map_err(|pool_err| EngineError::InternalError(format!("服务端遇到错误，无法取得与数据库的连接：{}。", pool_err)))?);

    let query =
        users
            .filter(
                name.eq(user.name).and(
                    password_hashed.eq(crate::hash::hash_password(&user.password[..]))
                )
            )
            .select(
                (id, name)
            );

    debug!("Login SQL: {}", diesel::debug_query::<diesel::pg::Pg, _>(&query));
            
    query
        .get_result::<RememberUserModel>(conn)
        .optional()
        .map_err(|db_err| EngineError::InternalError(format!("数据库查询失败：{}", db_err)))?
        .ok_or_else(|| EngineError::BadRequest(format!("没有该用户或密码错误。")))
}