use serde::Serialize;
use erased_serde::Serialize as ESerialize;
use std::error::Error as StdError;
use std::fmt::{self, Display};

#[derive(Debug)]
pub enum Error {
    UnknownEndpoint,
    InternalServer,
    RateLimit(RateLimitError),
    NotFound,
}

impl Error {
    pub fn to_status_code(&self) -> warp::http::StatusCode {
        match self {
            Error::UnknownEndpoint => warp::http::StatusCode::NOT_FOUND,
            Error::InternalServer => warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            Error::RateLimit(_) => warp::http::StatusCode::TOO_MANY_REQUESTS,
            Error::NotFound => warp::http::StatusCode::NOT_FOUND,
        }
    }

    pub fn to_flat_error<'a>(&'a self) -> FlatError<'a> {
        match self {
            Error::UnknownEndpoint => FlatError {
                error_code: 0,
                error_name: "unknownEndpoint",
                error_value: None,
            },
            Error::InternalServer => FlatError {
                error_code: 1,
                error_name: "internalServer",
                error_value: None,
            },
            Error::RateLimit(rate_limit_error) => FlatError {
                error_code: 2,
                error_name: "rateLimit",
                error_value: Some(rate_limit_error),
            },
            Error::NotFound => FlatError {
                error_code: 3,
                error_name: "notFound",
                error_value: None,
            },
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Error::UnknownEndpoint => "Unknown endpoint",
            Error::InternalServer => "Internal server",
            Error::RateLimit(_) => "Rate limited",
            Error::NotFound => "Not found",
        })
    }
}

impl StdError for Error {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatError<'a> {
    pub error_code: u16,
    pub error_name: &'static str,
    pub error_value: Option<&'a dyn ESerialize>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitError {
    pub wait_time_millis: u64,
}
