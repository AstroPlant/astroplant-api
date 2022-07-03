use axum::headers::HeaderName;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use bytes::Bytes;
use erased_serde::Serialize as ErasedSerialize;
use futures::stream::Stream;

use std::pin::Pin;
use std::str::FromStr;

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
    // TODO: make this Option to avoid allocation if no headers
    // are added
    headers: HeaderMap,
}

#[allow(dead_code)]
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
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }
}

impl IntoResponse for Response {
    fn into_response(mut self) -> axum::response::Response {
        let res = match self.value {
            Some(ResponseValue::Serializable(value)) => {
                Box::new((self.status_code, self.headers, Json(value)).into_response())
            }
            Some(ResponseValue::Data { media_type, data }) => {
                // FIXME: media type may be invalid
                self.headers.insert(
                    header::CONTENT_TYPE,
                    media_type.parse().expect("valid media type"),
                );
                Box::new((self.status_code, self.headers, data).into_response())
            }
            Some(ResponseValue::Stream { media_type, stream }) => {
                // FIXME: media type may be invalid
                self.headers.insert(
                    header::CONTENT_TYPE,
                    media_type.parse().expect("valid media type"),
                );
                Box::new(
                    (
                        self.status_code,
                        self.headers,
                        axum::body::StreamBody::new(stream),
                    )
                        .into_response(),
                )
            }
            None => Box::new((self.status_code, self.headers).into_response()),
        };
        res.into_response()
    }
}

pub struct ResponseBuilder {
    status_code: StatusCode,
    headers: HeaderMap,
    links: Vec<String>,
}

impl ResponseBuilder {
    /// Create a new response builder with the given status code.
    pub fn new(status_code: StatusCode) -> Self {
        ResponseBuilder {
            status_code,
            headers: HeaderMap::new(),
            links: Vec::new(),
        }
    }

    fn process(&mut self) {
        if !self.links.is_empty() {
            let links = self.links.join(", ");
            self.headers
                .insert(header::LINK, links.parse().expect("valid header value"));
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
    pub fn next_page_uri(mut self, uri: &str) -> Self {
        self.headers.insert(
            HeaderName::from_static("x-next"),
            uri.parse().expect("valid header value"),
        );
        self
    }

    /// Set the content disposition to attachment with the given file name.
    #[allow(dead_code)]
    pub fn attachment_filename(mut self, filename: &str) -> Self {
        self.headers.insert(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename={}", filename)
                .parse()
                .expect("valid header value"),
        );
        self
    }

    pub fn link(mut self, uri: &str, rel: &str) -> Self {
        self.links.push(format!("<{}>; rel=\"{}\"", uri, rel));
        self
    }

    /// Add a Location URI header. Only makes sense with the Created or a Redirection status.
    #[allow(dead_code)]
    pub fn content_uri(mut self, uri: &str) -> Self {
        self.headers
            .insert(header::LOCATION, uri.parse().expect("valid header value"));
        self
    }

    /// Set a response header.
    #[allow(dead_code)]
    pub fn header(mut self, header_name: &str, header_value: &str) -> Self {
        self.headers.insert(
            HeaderName::from_str(header_name).expect("valid header name"),
            header_value.parse().expect("valid header value"),
        );
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
