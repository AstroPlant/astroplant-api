use serde::Serialize;
use std::error::Error as StdError;
use std::fmt::{self, Display};

#[derive(Debug)]
pub enum Error {
    RateLimit(RateLimitError),
}

impl Error {
    pub fn to_status_code(&self) -> warp::http::StatusCode {
        match self {
            Error::RateLimit(_) => warp::http::StatusCode::TOO_MANY_REQUESTS,
        }
    }

    pub fn to_flat_error<'a>(&'a self) -> FlatError<'a, impl Serialize> {
        match self {
            Error::RateLimit(rate_limit_error) => FlatError {
                error_code: 1,
                error_name: "rateLimitError",
                error_value: Some(rate_limit_error),
            }
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Error::RateLimit(_) => "Rate limited",
        })
    }
}

impl StdError for Error {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlatError<'a, S: Serialize + 'a> {
    pub error_code: u16,
    pub error_name: &'static str,
    pub error_value: Option<&'a S>,
}

#[derive(Serialize, Debug)]
pub struct RateLimitError {
    pub wait_time_millis: u64,
}
