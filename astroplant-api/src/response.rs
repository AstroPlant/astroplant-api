use bytes::Bytes;
use erased_serde::Serialize as ErasedSerialize;
use futures::stream::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use warp::http::StatusCode;

pub enum ResponseValue {
    Serializable(Box<dyn ErasedSerialize + Send>),
    Data {
        media_type: String,
        data: Vec<u8>,
    },
    Stream {
        media_type: String,
        stream: Pin<
            Box<dyn Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + 'static>,
        >,
    },
}

pub struct Response {
    value: Option<ResponseValue>,
    status_code: StatusCode,
    headers: HashMap<String, String>,
}

impl Response {
    /// The response value.
    pub fn value(self) -> Option<ResponseValue> {
        self.value
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

pub struct ResponseBuilder {
    status_code: StatusCode,
    headers: HashMap<String, String>,
    links: Vec<String>,
}

impl ResponseBuilder {
    /// Create a new response builder with the given status code.
    pub fn new(status_code: StatusCode) -> Self {
        ResponseBuilder {
            status_code,
            headers: HashMap::new(),
            links: Vec::new(),
        }
    }

    fn process(&mut self) {
        if !self.links.is_empty() {
            let links = self.links.join(", ");
            self.headers.insert("link".to_owned(), links);
        }
    }

    /// Create a response with a 200 OK status code.
    #[allow(dead_code)]
    pub fn ok() -> Self {
        Self::new(StatusCode::OK)
    }

    /// Create a response with a 201 Created status code.
    #[allow(dead_code)]
    pub fn created() -> Self {
        Self::new(StatusCode::CREATED)
    }

    /// Add a (relative) next-page URI header to the response.
    #[allow(dead_code)]
    pub fn next_page_uri(mut self, uri: String) -> Self {
        self.headers.insert("x-next".to_owned(), uri);
        self
    }

    /// Set the content disposition to attachment with the given file name.
    #[allow(dead_code)]
    pub fn attachment_filename(mut self, filename: &str) -> Self {
        self.headers.insert(
            "Content-Disposition".to_owned(),
            format!("attachment; filename={}", filename),
        );
        self
    }

    pub fn link(mut self, uri: &str, rel: &str) -> Self {
        self.links.push(format!("<{}>; rel=\"{}\"", uri, rel));
        self
    }

    /// Add a Location URI header. Only makes sense with the Created or a Redirection status.
    #[allow(dead_code)]
    pub fn content_uri(mut self, uri: String) -> Self {
        self.headers.insert("Location".to_owned(), uri);
        self
    }

    /// Set a response header.
    #[allow(dead_code)]
    pub fn header(mut self, header_name: String, header_value: String) -> Self {
        self.headers.insert(header_name, header_value);
        self
    }

    /// Build an empty response.
    #[allow(dead_code)]
    pub fn empty(mut self) -> Response {
        self.process();
        Response {
            value: None,
            status_code: self.status_code,
            headers: self.headers,
        }
    }

    /// Build the response with the given serializable value.
    #[allow(dead_code)]
    pub fn body<T>(mut self, value: T) -> Response
    where
        T: ErasedSerialize + Send + 'static,
    {
        self.process();
        Response {
            value: Some(ResponseValue::Serializable(
                Box::new(value) as Box<dyn ErasedSerialize + Send>
            )),
            status_code: self.status_code,
            headers: self.headers,
        }
    }

    /// Build the response with the given raw data.
    #[allow(dead_code)]
    pub fn data(mut self, media_type: String, data: Vec<u8>) -> Response {
        self.process();
        Response {
            value: Some(ResponseValue::Data { media_type, data }),
            status_code: self.status_code,
            headers: self.headers,
        }
    }

    /// Build the response with the given raw data stream.
    #[allow(dead_code)]
    pub fn stream(
        mut self,
        media_type: String,
        stream: Pin<
            Box<dyn Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + 'static>,
        >,
    ) -> Response {
        self.process();
        Response {
            value: Some(ResponseValue::Stream { media_type, stream }),
            status_code: self.status_code,
            headers: self.headers,
        }
    }
}
