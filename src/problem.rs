//! The problems that can occur when using this API.
//! Implements RFC7807.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt::{self, Display};

pub const NOT_FOUND: Problem = Problem::Generic(GenericProblem::NotFound);
pub const INTERNAL_SERVER_ERROR: Problem = Problem::Generic(GenericProblem::InternalServerError);

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Problem {
    #[serde(rename = "about:blank")]
    Generic(GenericProblem),

    #[serde(rename = "/probs/rate-limit")]
    RateLimit(RateLimitError),

    #[serde(rename = "/probs/authorization-header")]
    AuthorizationHeader {
        category: AuthenticationTokenProblemCategory,
    },

    #[serde(rename = "/probs/payload-too-large")]
    #[serde(rename_all = "camelCase")]
    PayloadTooLarge { limit: u64 },

    #[serde(rename = "/probs/invalid-json")]
    #[serde(rename_all = "camelCase")]
    InvalidJson {
        category: JsonDeserializeErrorCategory,
    },

    #[serde(rename = "/probs/invalid-parameters")]
    #[serde(rename_all = "camelCase")]
    InvalidParameters {
        invalid_parameters: InvalidParameters,
    },
}

impl Problem {
    pub fn to_status_code(&self) -> warp::http::StatusCode {
        use GenericProblem::*;
        use Problem::*;

        match self {
            Generic(NotFound) => warp::http::StatusCode::NOT_FOUND,
            Generic(InternalServerError) => warp::http::StatusCode::INTERNAL_SERVER_ERROR,
            RateLimit(_) => warp::http::StatusCode::TOO_MANY_REQUESTS,
            AuthorizationHeader { .. } => warp::http::StatusCode::UNAUTHORIZED,
            PayloadTooLarge { .. } => warp::http::StatusCode::PAYLOAD_TOO_LARGE,
            InvalidJson { .. } => warp::http::StatusCode::BAD_REQUEST,
            InvalidParameters { .. } => warp::http::StatusCode::BAD_REQUEST,
        }
    }
}

impl Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let descriptive_problem: DescriptiveProblem = self.into();

        // TODO: improve
        f.write_str(&format!("{:?}", descriptive_problem))
    }
}

impl StdError for Problem {}

#[derive(Debug, Serialize, Deserialize)]
// Note: this attribute is a bit hacky, as DescriptiveProblem also defines a title field. But it
// works as expected (i.e., when Serde serializes a DescriptiveProblem with its title field set to
// None and with its problem field to the Problem::Generic variant, the title field in the generated
// serialization is taken from the GenericProblem variant).
//
// The reason for doing it like this, is to allow a Problem to be deserialized directly from a
// DescriptiveProblem serialization.
#[serde(tag = "title")]
pub enum GenericProblem {
    #[serde(rename = "Not Found")]
    NotFound,

    #[serde(rename = "Internal Server Error")]
    InternalServerError,
}

#[derive(Debug, Serialize)]
pub struct DescriptiveProblem<'a> {
    #[serde(flatten)]
    pub problem: &'a Problem,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl<'a> From<&'a Problem> for DescriptiveProblem<'a> {
    fn from(problem: &'a Problem) -> DescriptiveProblem<'a> {
        use Problem::*;

        let status = Some(problem.to_status_code().as_u16());

        let (title, detail) = match problem {
            Generic(_) => {
                (None, None)
            }

            RateLimit(_) => {
                (
                    Some("Your request has been rate limited.".to_owned()),
                    None,
                )
            }

            AuthorizationHeader { category } => {
                use AuthenticationTokenProblemCategory::*;
                match category {
                    Missing => (
                        Some("Your request misses the Authorization header.".to_owned()),
                        None,
                    ),
                    Malformed => (
                        Some("Your request Authorization header was malformed.".to_owned()),
                        None,
                    ),
                    Expired => (
                        Some("Your request authorization token was expired.".to_owned()),
                        None,
                    ),
                }
            }

            PayloadTooLarge { limit } => {
                (
                    Some("Your request payload was too large.".to_owned()),
                    Some(format!("The request payload limit was {} bytes.", limit)),
                )
            }

            InvalidJson { .. } => {
                (
                    Some("Your request JSON was malformed.".to_owned()),
                    Some("The JSON might be syntactically incorrect, or it might not adhere to the endpoint's schema. Refer to the category for more information.".to_owned()),
                )
            }

            InvalidParameters { .. } => {
                (
                    Some("Your request parameters did not validate.".to_owned()),
                    None,
                )
            }
        };

        DescriptiveProblem {
            problem,
            title,
            status,
            detail,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthenticationTokenProblemCategory {
    Missing,
    Malformed,
    Expired,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvalidParameters {
    #[serde(flatten)]
    inner: HashMap<std::borrow::Cow<'static, str>, Vec<InvalidParameterReason>>,
}

impl InvalidParameters {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.inner.iter().any(|(_, reasons)| !reasons.is_empty())
    }

    pub fn add<S: Into<std::borrow::Cow<'static, str>>>(
        &mut self,
        parameter: S,
        reason: InvalidParameterReason,
    ) {
        self.inner
            .entry(parameter.into())
            .or_insert(vec![])
            .push(reason)
    }
}

impl<E: std::borrow::Borrow<validator::ValidationErrors>> From<E> for InvalidParameters {
    fn from(validation_errors: E) -> InvalidParameters {
        use heck::MixedCase; // This is "camelCase" in Serde.
        let mut invalid_parameters = InvalidParameters::new();

        for (field, errors) in validation_errors.borrow().field_errors().into_iter() {
            for error in errors.into_iter() {
                invalid_parameters.add(field.to_mixed_case(), error.into())
            }
        }

        invalid_parameters
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvalidParameterReason {
    MustBeEmailAddress,
    MustBeUrl,
    MustBeInRange { min: f64, max: f64 },
    MustHaveLengthBetween { min: u64, max: u64 },
    MustHaveLengthExactly { length: u64 },
    AlreadyExists,
    InvalidToken { category: AuthenticationTokenProblemCategory },
    Other,
}

impl<E: std::borrow::Borrow<validator::ValidationError>> From<E> for InvalidParameterReason {
    fn from(validation_error: E) -> InvalidParameterReason {
        use InvalidParameterReason::*;

        let validation_error: &validator::ValidationError = validation_error.borrow();

        match validation_error.code.as_ref() {
            "email" => MustBeEmailAddress,
            "url" => MustBeUrl,
            "range" => {
                let min: Option<f64> = validation_error
                    .params
                    .get("min")
                    .map(|v| v.as_f64().unwrap());
                let max: Option<f64> = validation_error
                    .params
                    .get("max")
                    .map(|v| v.as_f64().unwrap());

                match (min, max) {
                    (Some(min), Some(max)) => MustBeInRange { min, max },
                    _ => Other,
                }
            }
            "length" => {
                let min: Option<u64> = validation_error
                    .params
                    .get("min")
                    .map(|v| v.as_u64().unwrap());
                let max: Option<u64> = validation_error
                    .params
                    .get("max")
                    .map(|v| v.as_u64().unwrap());
                let equal: Option<u64> = validation_error
                    .params
                    .get("equal")
                    .map(|v| v.as_u64().unwrap());

                match (min, max, equal) {
                    (Some(min), Some(max), None) => MustHaveLengthBetween { min, max },
                    (None, None, Some(equal)) => MustHaveLengthExactly { length: equal },
                    _ => Other,
                }
            }
            _ => Other,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitError {
    pub wait_time_millis: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JsonDeserializeErrorCategory {
    Syntactic,
    Semantic,
    PrematureEnd,
    Other,
}

impl From<serde_json::error::Category> for JsonDeserializeErrorCategory {
    fn from(category: serde_json::error::Category) -> Self {
        use serde_json::error::Category::*;

        match category {
            Syntax => Self::Syntactic,
            Data => Self::Semantic,
            Eof => Self::PrematureEnd,
            _ => Self::Other,
        }
    }
}

impl From<&serde_json::Error> for JsonDeserializeErrorCategory {
    fn from(error: &serde_json::Error) -> Self {
        error.classify().into()
    }
}
