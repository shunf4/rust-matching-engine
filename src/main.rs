#![ allow( dead_code, unused_imports ) ]


#[macro_use]
extern crate diesel;
#[macro_use]
extern crate serde_derive;
extern crate dotenv;
extern crate listenfd;
#[macro_use]
extern crate log;
extern crate env_logger;

pub mod schema;
pub mod models;
pub mod hash;
pub mod errors;
pub mod handlers;
pub mod common;

use errors::EngineError;
use diesel::prelude::*;
use diesel::pg::PgConnection;
use std::env;
use diesel::r2d2;
use diesel::r2d2::ConnectionManager;
use actix_web::middleware;
use actix_web::guard;
use actix_identity::{CookieIdentityPolicy, IdentityService};
use actix_web::FromRequest;

use listenfd::ListenFd;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};

const DEFAULT_SECRET_KEY : &str = "hhxxsjnbhhxxsjnbhhxxsjnbhhxxsjnb";

pub fn test_get_data_connection() -> PgConnection {
    dotenv::dotenv().ok();    // 引入本目录下 .env 文件作为环境变量

    let database_url = std::env::var("DATABASE_URL").expect("必须设置环境变量（也可在 .env 中填写） DATABASE_URL=PostgreSQL数据库连接URL！");
    
    diesel::pg::PgConnection::establish(&database_url).expect(&format!("连接数据库 {} 失败！", database_url))
}

fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();    // 引入本目录下 .env 文件作为环境变量

    // 初始化日志
    std::env::set_var(
        "RUST_LOG",
        "rust_matching_engine=debug,actix_web=debug,actix_server=debug",
    );
    std::env::set_var(
        "RUST_BACKTRACE",
        "1",
    );
    env_logger::init();

    // 初始化数据库连接
    let database_url = std::env::var("DATABASE_URL").expect("必须设置环境变量（也可在 .env 中填写） DATABASE_URL=PostgreSQL数据库连接URL！");

        // 线程池
    let conn_man = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder().build(conn_man).expect("创建数据库连接线程池失败！");

    // 创建在调试环境下可以即修改代码即重启的监听描述器，并创建 HTTP 服务器

    let mut listenfd = ListenFd::from_env();
    
    let mut server = HttpServer::new(move || {
        App::new()
            .data(pool.clone())     // 每个传入的 HTTP 连接，都先从数据库线程池取出一条连接，附加到应用附加数据中
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(
                    match std::env::var("SECRET_KEY") {
                        Ok(key) => String::from(&key),
                        Err(_) => DEFAULT_SECRET_KEY.to_owned()
                    }.as_bytes()    // Cookie 会根据密钥加密用户的验证信息
                )
                    .name("stock-login-token")
                    .path("/stock-api")
                    .max_age_time(chrono::Duration::days(3))
                    .secure(false)
            ))      // 过一个身份验证中间件
            .data(web::Json::<crate::handlers::users::RegisterModel>::configure(|cfg| {
                cfg.error_handler(|err, _| {
                    EngineError::BadRequest(format!("解析传来的 JSON 数据错误：{}。请检查数据。", err)).into()
                })
            }))   // 给 Json parser 添加配置
            .data(web::Path::<u32>::configure(|cfg| {
                cfg.error_handler(|err, _| {
                    EngineError::BadRequest(format!("解析路径中整数错误：{}。请检查数据。", err)).into()
                })
            }))   // 给 路径参数 Parser 添加配置
            .data(web::Query::<crate::handlers::PagingModel>::configure(|cfg| {
                cfg.error_handler(|err, _| {
                    EngineError::BadRequest(format!("解析 Query String 中翻页数据错误：{}。请检查数据。", err)).into()
                })
            }))   // 给 请求参数 Query Parser 添加配置
            .service(
                web::scope("/stock-api/v1")
                    .service(
                        handlers::users::make_scope()
                    )
                    .service(
                        web::resource("/auth")
                            .route(web::post().to_async(handlers::users::login))
                            .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
                    )
                    .service(
                        handlers::stocks::make_scope()
                    )
                    .service(
                        web::resource("/recharge")
                            .route(web::post().to_async(handlers::recharge::recharge))
                            .to(|| Err::<(), EngineError>(EngineError::MethodNotAllowed(format!("错误：不允许此 HTTP 谓词。"))))
                    )
                    .default_service(
                        web::route().to(
                            || Err::<(), EngineError>(EngineError::NotFound(format!("错误：未找到资源。")))
                        )
                    )
            )
            .wrap(middleware::Logger::default())    //过一个日志中间件
    });

    server = if let Some(l) = listenfd.take_tcp_listener(0).unwrap() {
        server.listen(l).unwrap()
    } else {
        server.bind("0.0.0.0:7878").unwrap()
    };

    server.run()
}


#[test]
fn test_data_connection() {
    test_get_data_connection();
}

