//! Client for the [NameGender](https://namegender.com) API.
//!
//! ```no_run
//! use namegender::{NameGender, Options};
//!
//! # async fn run() -> Result<(), namegender::Error> {
//! let client = NameGender::new(std::env::var("NAMEGENDER_API_KEY").unwrap());
//! let result = client
//!     .name("Andrea", &Options { country: Some("IT".into()), ..Default::default() })
//!     .await?;
//! println!("{:?} {} {}", result.gender, result.probability, result.sample_size);
//! # Ok(())
//! # }
//! ```
//!
//! Every lookup costs one credit, unknown results included. Success is the
//! HTTP status; any other response is returned as [`Error::Api`] carrying the
//! API's reason code.

use serde::{Deserialize, Serialize};
use std::fmt;

const DEFAULT_BASE_URL: &str = "https://namegender.com/api/v1";

/// Options shared by the name, email, username and bulk lookups.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Options {
    /// ISO 3166-1 alpha-2 code. Andrea is male in Italy and female in Germany.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Language tag such as `it-IT`; its region is the country when `country`
    /// is not sent. A tag without a region (`en`) sets none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// End user IP address; its country is used when neither `country` nor a
    /// regional `locale` is sent. Not stored by the API.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// Return the more likely gender even when the evidence is weak.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub best_guess: bool,
    /// Ask a language model when the dataset has no answer. Requires AI
    /// lookups to be enabled on the account.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ai_fallback: bool,
}

/// How the values of a bulk request are read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    #[default]
    Name,
    Email,
    Username,
}

/// One resolved value.
#[derive(Debug, Clone, Deserialize)]
pub struct GenderResult {
    pub query: String,
    pub name: Option<String>,
    pub first_name: Option<String>,
    pub middle_name: Option<String>,
    pub last_name: Option<String>,
    pub name_type: Option<String>,
    /// `"male"`, `"female"` or `None` when the API does not know.
    pub gender: Option<String>,
    pub country: Option<String>,
    /// How many real people the answer is based on; 0 when the source
    /// records proportions rather than counts.
    pub sample_size: u64,
    pub probability: u8,
    pub took_ms: u64,
    pub source: String,
    pub confidence: String,
    pub matched_as: Option<String>,
}

/// Response of a single lookup.
#[derive(Debug, Clone, Deserialize)]
pub struct GenderResponse {
    pub credits_charged: u64,
    pub credits_remaining: u64,
    pub data_version: Option<String>,
    pub request_id: Option<String>,
    /// Where the country came from: `"country"`, `"locale"`, `"ip"` or `None`.
    pub country_source: Option<String>,
    #[serde(flatten)]
    pub result: GenderResult,
}

impl std::ops::Deref for GenderResponse {
    type Target = GenderResult;
    fn deref(&self) -> &GenderResult {
        &self.result
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BulkSummary {
    pub total: u64,
    pub identified: u64,
    pub unknown: u64,
    pub match_rate: f64,
}

/// Response of a bulk lookup; `results` is in the order the values were sent.
#[derive(Debug, Clone, Deserialize)]
pub struct BulkResponse {
    pub credits_charged: u64,
    pub credits_remaining: u64,
    pub data_version: Option<String>,
    pub request_id: Option<String>,
    pub country_source: Option<String>,
    pub took_ms: u64,
    pub summary: BulkSummary,
    pub results: Vec<GenderResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Registration {
    pub country: String,
    pub count: u64,
    pub share: f64,
    pub gender: Option<String>,
    pub probability: u8,
    pub source: String,
}

/// Which countries a name is recorded in. Not a country-of-origin or
/// ethnicity inference: show `basis["note"]` next to any percentage.
#[derive(Debug, Clone, Deserialize)]
pub struct CountriesResponse {
    pub credits_charged: u64,
    pub credits_remaining: u64,
    pub registrations: Vec<Registration>,
    pub attested_in: Vec<String>,
    pub basis: serde_json::Value,
}

/// Credit balance and account status. Costs no credit.
#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    pub email: String,
    pub credits_remaining: u64,
    pub free_today: u64,
    pub free_daily_limit: u64,
}

/// Options for the salutation endpoints. Fields left as `None` are not sent.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SalutationOptions {
    /// Language of the salutation: `en`, `en-US`, `en-GB`, `de`, `de-AT`,
    /// `de-CH`, `fr`, `es`, `it`, `pt`, `pt-PT`, `pt-BR`, `nl`, `tr`, `pl` or
    /// `ja`. Defaults to the language of `locale`, else the main language of
    /// the country, else English. Any other value is a 422 `invalid_input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Country hint for the gender lookup, as in [`Options`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// A gender you already know: `male`, `female` or `neutral`. Overrides
    /// the lookup; `neutral` always gives the neutral form.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    /// Lowest probability (50-100) for a gendered salutation; the API
    /// default is 90. Below it the neutral form is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_probability: Option<u8>,
    /// Academic title kept in a separate field, such as `Dr.`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// The three ready lines. `neutral` is always the gender-free form.
#[derive(Debug, Clone, Deserialize)]
pub struct SalutationLines {
    pub formal: String,
    pub informal: String,
    pub neutral: String,
}

/// Pieces of the formal line, for building your own template.
#[derive(Debug, Clone, Deserialize)]
pub struct SalutationParts {
    pub opening: Option<String>,
    pub courtesy: Option<String>,
    pub academic: Option<String>,
    pub name: Option<String>,
}

/// The salutation for one name.
#[derive(Debug, Clone, Deserialize)]
pub struct SalutationResult {
    pub query: String,
    pub language: String,
    /// `"gendered"`, `"neutral"` or `"organization"`.
    pub form: String,
    /// Why the neutral form was used, such as `"gender_unknown"` or
    /// `"below_min_probability"`; `None` for the other forms.
    pub reason: Option<String>,
    pub salutation: SalutationLines,
    pub parts: SalutationParts,
    pub gender: Option<String>,
    /// `"lookup"`, `"input"`, `"title"` or `None`.
    pub gender_source: Option<String>,
    pub probability: Option<u8>,
    pub confidence: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub name_type: String,
    pub country: Option<String>,
}

/// Response of a single salutation.
#[derive(Debug, Clone, Deserialize)]
pub struct SalutationResponse {
    pub credits_charged: u64,
    pub credits_remaining: u64,
    pub data_version: Option<String>,
    pub request_id: Option<String>,
    pub country_source: Option<String>,
    #[serde(flatten)]
    pub result: SalutationResult,
}

impl std::ops::Deref for SalutationResponse {
    type Target = SalutationResult;
    fn deref(&self) -> &SalutationResult {
        &self.result
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SalutationSummary {
    pub total: u64,
    pub gendered: u64,
    pub neutral: u64,
    pub organization: u64,
}

/// Response of a bulk salutation; `results` is in the order the names were
/// sent.
#[derive(Debug, Clone, Deserialize)]
pub struct SalutationBulkResponse {
    pub credits_charged: u64,
    pub credits_remaining: u64,
    pub data_version: Option<String>,
    pub request_id: Option<String>,
    pub country_source: Option<String>,
    pub took_ms: u64,
    pub language: String,
    pub summary: SalutationSummary,
    pub results: Vec<SalutationResult>,
}

/// Options for the name check endpoints. Fields left as `None` are not sent.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NameCheckOptions {
    /// Country hint, as in [`Options`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

/// One reason behind an assessment.
#[derive(Debug, Clone, Deserialize)]
pub struct NameCheckSignal {
    /// Such as `"keyboard_pattern"`, `"placeholder"` or `"first_name_attested"`.
    pub code: String,
    /// `"high"`, `"medium"`, `"low"`, `"info"` or `"positive"`.
    pub severity: String,
    /// `"full"`, `"first_name"`, `"last_name"` or `None`.
    pub part: Option<String>,
    pub value: Option<String>,
}

/// What the database knows about the first name. Surnames are judged by
/// their shape only.
#[derive(Debug, Clone, Deserialize)]
pub struct NameCheckEvidence {
    /// `"counted"`, `"attested"`, `"not_found"` or `None`.
    pub first_name_status: Option<String>,
    pub first_name_counted_records: u64,
}

/// Whether one name looks like a real person's name. It never calls a name
/// fake: use it to flag records, not to reject people automatically.
#[derive(Debug, Clone, Deserialize)]
pub struct NameCheckResult {
    pub query: String,
    /// `"plausible"`, `"suspicious"` or `"implausible"`.
    pub assessment: String,
    /// 0-100.
    pub score: u8,
    pub signals: Vec<NameCheckSignal>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    /// `"personal"`, `"organization"` or `"role"`.
    pub name_type: String,
    pub evidence: NameCheckEvidence,
}

/// Response of a single name check.
#[derive(Debug, Clone, Deserialize)]
pub struct NameCheckResponse {
    pub credits_charged: u64,
    pub credits_remaining: u64,
    pub data_version: Option<String>,
    pub request_id: Option<String>,
    pub country_source: Option<String>,
    #[serde(flatten)]
    pub result: NameCheckResult,
}

impl std::ops::Deref for NameCheckResponse {
    type Target = NameCheckResult;
    fn deref(&self) -> &NameCheckResult {
        &self.result
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NameCheckSummary {
    pub total: u64,
    pub plausible: u64,
    pub suspicious: u64,
    pub implausible: u64,
}

/// Response of a bulk name check; `results` is in the order the names were
/// sent.
#[derive(Debug, Clone, Deserialize)]
pub struct NameCheckBulkResponse {
    pub credits_charged: u64,
    pub credits_remaining: u64,
    pub data_version: Option<String>,
    pub request_id: Option<String>,
    pub country_source: Option<String>,
    pub took_ms: u64,
    pub summary: NameCheckSummary,
    pub results: Vec<NameCheckResult>,
}

#[derive(Debug)]
pub enum Error {
    /// The API answered with a non-2xx status. `error` is the reason code to
    /// branch on, such as `no_credits` or `invalid_key`.
    Api {
        status: u16,
        error: String,
        message: String,
        request_id: Option<String>,
    },
    /// The request did not complete, or the response could not be read.
    Http(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Api {
                status,
                error,
                message,
                ..
            } => write!(f, "{message} ({error}, HTTP {status})"),
            Error::Http(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Http(e)
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    error: String,
    #[serde(default)]
    message: String,
    request_id: Option<String>,
}

/// The API client. Cheap to clone; reuse one per process.
#[derive(Debug, Clone)]
pub struct NameGender {
    api_key: String,
    base_url: String,
    http: reqwest::Client,
}

impl NameGender {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// Point the client at another base URL, for example a test server.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    pub async fn name(&self, name: &str, options: &Options) -> Result<GenderResponse, Error> {
        self.post("/gender", &with_field("name", name, options))
            .await
    }

    pub async fn email(&self, email: &str, options: &Options) -> Result<GenderResponse, Error> {
        self.post("/gender/email", &with_field("email", email, options))
            .await
    }

    pub async fn username(
        &self,
        username: &str,
        options: &Options,
    ) -> Result<GenderResponse, Error> {
        self.post(
            "/gender/username",
            &with_field("username", username, options),
        )
        .await
    }

    /// Up to 100 values in one request, one credit each.
    pub async fn bulk<S: AsRef<str>>(
        &self,
        values: &[S],
        value_type: ValueType,
        options: &Options,
    ) -> Result<BulkResponse, Error> {
        let mut body = serde_json::to_value(options).expect("options serialize");
        body["names"] = values.iter().map(|v| v.as_ref()).collect::<Vec<_>>().into();
        body["type"] = serde_json::to_value(value_type).expect("type serializes");
        self.post("/gender/bulk", &body).await
    }

    pub async fn countries(&self, name: &str, limit: u8) -> Result<CountriesResponse, Error> {
        self.post(
            "/gender/countries",
            &serde_json::json!({ "name": name, "limit": limit.clamp(1, 100) }),
        )
        .await
    }

    /// The letter salutation for a full name, titles included
    /// (`Dr. Anna Müller`). One credit. The form is neutral when the gender
    /// is not certain; `form` and `reason` say why.
    pub async fn salutation(
        &self,
        name: &str,
        options: &SalutationOptions,
    ) -> Result<SalutationResponse, Error> {
        let mut body = serde_json::to_value(options).expect("options serialize");
        body["name"] = name.into();
        self.post("/salutation", &body).await
    }

    /// Like [`salutation`](Self::salutation) for a first and last name stored
    /// separately; they are not parsed. An empty string is not sent.
    pub async fn salutation_by_parts(
        &self,
        first_name: &str,
        last_name: &str,
        options: &SalutationOptions,
    ) -> Result<SalutationResponse, Error> {
        let mut body = serde_json::to_value(options).expect("options serialize");
        for (field, value) in [("first_name", first_name), ("last_name", last_name)] {
            if !value.is_empty() {
                body[field] = value.into();
            }
        }
        self.post("/salutation", &body).await
    }

    /// Salutations for up to 100 names in one request, one credit each.
    pub async fn salutation_bulk<S: AsRef<str>>(
        &self,
        names: &[S],
        options: &SalutationOptions,
    ) -> Result<SalutationBulkResponse, Error> {
        let mut body = serde_json::to_value(options).expect("options serialize");
        body["names"] = names.iter().map(|n| n.as_ref()).collect::<Vec<_>>().into();
        self.post("/salutation/bulk", &body).await
    }

    /// Whether a name typed into a form looks like a real person's name,
    /// with the reasons. One credit. It never calls a name fake: use it to
    /// flag records, not to reject people automatically.
    pub async fn name_check(
        &self,
        name: &str,
        options: &NameCheckOptions,
    ) -> Result<NameCheckResponse, Error> {
        let mut body = serde_json::to_value(options).expect("options serialize");
        body["name"] = name.into();
        self.post("/name-check", &body).await
    }

    /// Like [`name_check`](Self::name_check) for a first and last name stored
    /// separately; they are not parsed. An empty string is not sent.
    pub async fn name_check_by_parts(
        &self,
        first_name: &str,
        last_name: &str,
        options: &NameCheckOptions,
    ) -> Result<NameCheckResponse, Error> {
        let mut body = serde_json::to_value(options).expect("options serialize");
        for (field, value) in [("first_name", first_name), ("last_name", last_name)] {
            if !value.is_empty() {
                body[field] = value.into();
            }
        }
        self.post("/name-check", &body).await
    }

    /// Checks up to 100 names in one request, one credit each.
    pub async fn name_check_bulk<S: AsRef<str>>(
        &self,
        names: &[S],
        options: &NameCheckOptions,
    ) -> Result<NameCheckBulkResponse, Error> {
        let mut body = serde_json::to_value(options).expect("options serialize");
        body["names"] = names.iter().map(|n| n.as_ref()).collect::<Vec<_>>().into();
        self.post("/name-check/bulk", &body).await
    }

    pub async fn account(&self) -> Result<Account, Error> {
        let request = self.http.get(format!("{}/me", self.base_url));
        self.send(request).await
    }

    async fn post<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T, Error> {
        let request = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .json(body);
        self.send(request).await
    }

    async fn send<T: for<'de> Deserialize<'de>>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, Error> {
        let response = request
            .bearer_auth(&self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            return Ok(response.json::<T>().await?);
        }

        let text = response.text().await.unwrap_or_default();
        let body: Option<ErrorBody> = serde_json::from_str(&text).ok();
        Err(match body {
            Some(b) => Error::Api {
                status: status.as_u16(),
                message: if b.message.is_empty() {
                    format!("HTTP {}", status.as_u16())
                } else {
                    b.message
                },
                error: b.error,
                request_id: b.request_id,
            },
            None => Error::Api {
                status: status.as_u16(),
                error: "http_error".into(),
                message: format!("HTTP {}", status.as_u16()),
                request_id: None,
            },
        })
    }
}

fn with_field(field: &str, value: &str, options: &Options) -> serde_json::Value {
    let mut body = serde_json::to_value(options).expect("options serialize");
    body[field] = value.into();
    body
}
