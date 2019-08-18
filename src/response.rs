use std::collections::HashMap;
use erased_serde::Serialize as ErasedSerialize;
use warp::http::StatusCode;

pub struct Response {
    value: Box<dyn ErasedSerialize + Send>,
    status_code: StatusCode,
    headers: HashMap<String, String>,
}

impl Response {
    /// Create a response with a 200 OK status code.
    pub fn ok<T: ErasedSerialize + Send + 'static>(value: T) -> Self {
        Response {
            value: Box::new(value),
            status_code: StatusCode::OK,
            headers: HashMap::new(),
        }
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
    pub fn value(&self) -> &(dyn ErasedSerialize + Send) {
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
