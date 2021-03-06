use actix_web::{error, HttpResponse};
use derive_more::Display;
use failure::Fail;
use http::StatusCode;

#[derive(Serialize)]
pub struct EngineErrorModel {
    status: u16,
    error: String,
}

#[derive(Debug, Display)]
pub enum EngineError {
    #[display(fmt = "{}", _0)]
    InternalError(String),
    #[display(fmt = "{}", _0)]
    BadRequest(String),
    #[display(fmt = "{}", _0)]
    Insufficient(crate::handlers::orders::OrderResult),
    #[display(fmt = "{}", _0)]
    Unauthorized(String),
    #[display(fmt = "{}", _0)]
    MethodNotAllowed(String),
    #[display(fmt = "{}", _0)]
    NotFound(String),
}

impl error::ResponseError for EngineError {
    // fn error_response(&self) -> HttpResponse {
    //     // 废弃
    //     match self {
    //         EngineError::InternalError(_) => HttpResponse::InternalServerError().finish(),
    //         EngineError::BadRequest(_) => HttpResponse::BadRequest().finish(),
    //         EngineError::Unauthorized(_) => HttpResponse::Unauthorized().finish(),
    //     }
    // }

    fn render_response(&self) -> HttpResponse {
        match self {
            EngineError::InternalError(m) => 
                HttpResponse::InternalServerError()
                    .json(EngineErrorModel {
                        error: m.to_owned(),
                        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16()
                    }),
            EngineError::BadRequest(m) => 
                HttpResponse::BadRequest()
                    .json(EngineErrorModel {
                        error: m.to_owned(),
                        status: StatusCode::BAD_REQUEST.as_u16()
                    }),
            EngineError::Unauthorized(m) => 
                HttpResponse::Unauthorized()
                    .json(EngineErrorModel {
                        error: m.to_owned(),
                        status: StatusCode::UNAUTHORIZED.as_u16()
                    }),
            EngineError::Insufficient(m) => 
                HttpResponse::NotAcceptable()
                    .json(m),
            EngineError::MethodNotAllowed(m) => 
                HttpResponse::MethodNotAllowed()
                    .json(EngineErrorModel {
                        error: m.to_owned(),
                        status: StatusCode::METHOD_NOT_ALLOWED.as_u16()
                    }),
            EngineError::NotFound(m) => 
                HttpResponse::NotFound()
                    .json(EngineErrorModel {
                        error: m.to_owned(),
                        status: StatusCode::NOT_FOUND.as_u16()
                    }),
        }
    }
}

impl From<diesel::result::Error> for EngineError {
    fn from(e: diesel::result::Error) -> EngineError {
        EngineError::InternalError(format!("数据库事务错误：{}", e))
    }
}