use erased_serde::Serialize as ErasedSerialize;
use serde::Serialize;
use std::collections::HashMap;
use warp::http::StatusCode;

#[derive(Serialize)]
pub enum Never {}

pub struct Response {
    value: Option<Box<dyn ErasedSerialize + Send>>,
    status_code: StatusCode,
    headers: HashMap<String, String>,
}

impl Response {
    pub fn new<T>(
        value: Option<T>,
        status_code: StatusCode,
        headers: HashMap<String, String>,
    ) -> Self
    where
        T: ErasedSerialize + Send + 'static,
    {
        Response {
            value: value.map(|v: T| Box::new(v) as Box<dyn ErasedSerialize + Send>),
            status_code,
            headers,
        }
    }

    /// Create a response with a 200 OK status code.
    pub fn ok<T: ErasedSerialize + Send + 'static>(value: T) -> Self {
        Self::new(Some(value), StatusCode::OK, HashMap::new())
    }

    pub fn ok_empty() -> Self {
        Self::new::<Never>(None, StatusCode::OK, HashMap::new())
    }

    pub fn created<T: ErasedSerialize + Send + 'static>(value: T) -> Self {
        Self::new(Some(value), StatusCode::CREATED, HashMap::new())
    }

    pub fn created_empty() -> Self {
        Self::new::<Never>(None, StatusCode::CREATED, HashMap::new())
    }

    /// Add a (relative) next-page URI header to the response.
    pub fn set_next_page_uri(&mut self, uri: String) {
        self.headers.insert("x-next".to_owned(), uri);
    }

    /// Set a response header.
    pub fn set_header(&mut self, header_name: String, header_value: String) {
        self.headers.insert(header_name, header_value);
    }

    /// The response value.
    pub fn value(&self) -> &Option<Box<dyn ErasedSerialize + Send>> {
        &self.value
    }

    /// The response status code.
    pub fn status_code(&self) -> StatusCode {
        self.status_code
    }

    /// The response headers.
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}
