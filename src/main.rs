use warp::{self, path, Filter, Rejection, Reply};

mod error;
use error::Error;
mod rate_limit;

fn main() {
    let hello = path!("hello" / String)
        .and(rate_limit::leaky_bucket())
        .map(|name| format!("Hello, {}!", name))
        .recover(handle_rejection);

    warp::serve(hello).run(([127, 0, 0, 1], 8080));
}

fn handle_rejection(rejection: Rejection) -> Result<impl Reply, Rejection> {
    if let Some(err) = rejection.find_cause::<Error>() {
        Ok(warp::reply::with_status(
            serde_json::to_string(&err.to_flat_error()).unwrap(),
            err.to_status_code(),
        ))
    } else {
        Err(rejection)
    }
}
