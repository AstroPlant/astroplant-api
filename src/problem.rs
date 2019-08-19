//! The problems that can occur when using this API.
//! Implements RFC7807.

use serde::{Deserialize, Serialize};
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
        invalid_parameters: Vec<InvalidParameter>
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
pub enum InvalidParameterReason {
    MustHaveLengthBetween { min: usize, max: usize },
    MustBeVariantOf(Vec<String>),
    AlreadyExists,
    // MustBeBetween({ min: i64, max: i64}),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidParameter {
    name: String,
    reason: InvalidParameterReason,
}

impl InvalidParameter {
    pub fn new(name: String, reason: InvalidParameterReason) -> Self {
        InvalidParameter { name, reason }
    }

    pub fn already_exists(name: String) -> Self {
        InvalidParameter {
            name,
            reason: InvalidParameterReason::AlreadyExists,
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
