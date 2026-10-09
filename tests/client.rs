// The salutation bodies are large json! literals.
#![recursion_limit = "256"]

use namegender::{
    AgeOptions, Error, NameCheckOptions, NameGender, Options, SalutationOptions, ValueType,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn result(query: &str, gender: Option<&str>, country: Option<&str>) -> serde_json::Value {
    json!({
        "query": query, "name": query, "first_name": query, "middle_name": null, "last_name": null,
        "name_type": "personal", "gender": gender, "country": country, "sample_size": 12345,
        "probability": 95, "took_ms": 1, "source": "db", "confidence": "high", "matched_as": null
    })
}

fn envelope() -> serde_json::Value {
    json!({ "credits_charged": 1, "credits_remaining": 99, "data_version": "2026.10", "request_id": "req_1" })
}

#[tokio::test]
async fn name_sends_options_and_key() {
    let server = MockServer::start().await;
    let mut body = envelope();
    body.as_object_mut().unwrap().extend(
        result("Andrea", Some("male"), Some("IT"))
            .as_object()
            .unwrap()
            .clone(),
    );
    body["country_source"] = json!("locale");

    Mock::given(method("POST"))
        .and(path("/gender"))
        .and(header("authorization", "Bearer ng_live_test"))
        .and(body_json(json!({ "name": "Andrea", "locale": "it-IT" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = Options {
        locale: Some("it-IT".into()),
        ..Default::default()
    };
    let r = client.name("Andrea", &options).await.unwrap();

    assert_eq!(r.gender.as_deref(), Some("male"));
    assert_eq!(r.country.as_deref(), Some("IT"));
    assert_eq!(r.country_source.as_deref(), Some("locale"));
    assert_eq!(r.credits_remaining, 99);
}

#[tokio::test]
async fn default_options_send_only_the_value() {
    let server = MockServer::start().await;
    let mut body = envelope();
    body.as_object_mut().unwrap().extend(
        result("jane", Some("female"), None)
            .as_object()
            .unwrap()
            .clone(),
    );
    body["country_source"] = json!(null);

    Mock::given(method("POST"))
        .and(path("/gender/email"))
        .and(body_json(json!({ "email": "jane.doe@example.com" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let r = client
        .email("jane.doe@example.com", &Options::default())
        .await
        .unwrap();
    assert_eq!(r.country_source, None);
}

#[tokio::test]
async fn bulk_sends_names_type_and_country() {
    let server = MockServer::start().await;
    let mut body = envelope();
    body["took_ms"] = json!(2);
    body["country_source"] = json!("country");
    body["summary"] = json!({ "total": 2, "identified": 1, "unknown": 1, "match_rate": 50.0 });
    body["results"] = json!([
        result("Emma", Some("female"), Some("DE")),
        result("Qzzxvv", None, Some("DE"))
    ]);

    Mock::given(method("POST"))
        .and(path("/gender/bulk"))
        .and(body_json(json!({ "names": ["Emma", "Qzzxvv"], "type": "name", "country": "DE", "best_guess": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = Options {
        country: Some("DE".into()),
        best_guess: true,
        ..Default::default()
    };
    let r = client
        .bulk(&["Emma", "Qzzxvv"], ValueType::Name, &options)
        .await
        .unwrap();

    assert_eq!(r.results.len(), 2);
    assert_eq!(r.results[1].gender, None);
    assert_eq!(r.summary.unknown, 1);
}

#[tokio::test]
async fn api_errors_carry_the_reason_code() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/gender"))
        .respond_with(ResponseTemplate::new(402).set_body_json(json!({
            "error": "no_credits", "message": "No credits left.", "request_id": "req_9"
        })))
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let err = client.name("Emma", &Options::default()).await.unwrap_err();

    match err {
        Error::Api {
            status,
            error,
            request_id,
            ..
        } => {
            assert_eq!(status, 402);
            assert_eq!(error, "no_credits");
            assert_eq!(request_id.as_deref(), Some("req_9"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn a_non_json_error_still_reports_the_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(502).set_body_string("Bad Gateway"))
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri() + "/");
    match client.account().await.unwrap_err() {
        Error::Api { status, error, .. } => {
            assert_eq!(status, 502);
            assert_eq!(error, "http_error");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn account_and_countries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "email": "dev@example.com", "credits_remaining": 99, "purchased_credits": 0,
            "free_today": 50, "free_daily_limit": 50, "lifetime_requests": 3, "data_version": "2026.10", "ai": {}
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/gender/countries"))
        .and(body_json(json!({ "name": "Mehmet", "limit": 100 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "credits_charged": 1, "credits_remaining": 98,
            "registrations": [{ "country": "FR", "count": 3775, "share": 58.97, "gender": "male", "probability": 99, "source": "insee" }],
            "attested_in": ["FR", "TR"], "basis": { "note": "Counted countries only." }
        })))
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    assert_eq!(client.account().await.unwrap().free_today, 50);

    // A limit above 100 is clamped rather than rejected by the API.
    let dist = client.countries("Mehmet", 200).await.unwrap();
    assert_eq!(dist.registrations[0].country, "FR");
    assert_eq!(dist.basis["note"], "Counted countries only.");
}

#[tokio::test]
async fn salutation_sends_only_set_options() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 1, "credits_remaining": 4999, "data_version": "2026.10", "request_id": "req_1",
        "country_source": "country", "query": "Dr. Anna Müller", "language": "de", "form": "gendered", "reason": null,
        "salutation": { "formal": "Sehr geehrte Frau Dr. Müller,", "informal": "Liebe Anna,", "neutral": "Guten Tag Dr. Anna Müller," },
        "parts": { "opening": "Sehr geehrte", "courtesy": "Frau", "academic": "Dr.", "name": "Müller" },
        "gender": "female", "gender_source": "lookup", "probability": 99, "confidence": "high",
        "first_name": "Anna", "last_name": "Müller", "name_type": "personal", "country": "DE"
    });

    Mock::given(method("POST"))
        .and(path("/salutation"))
        .and(header("authorization", "Bearer ng_live_test"))
        .and(body_json(json!({ "name": "Dr. Anna Müller", "language": "de", "country": "DE", "min_probability": 95 })))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = SalutationOptions {
        language: Some("de".into()),
        country: Some("DE".into()),
        min_probability: Some(95),
        ..Default::default()
    };
    let name = "Dr. Anna Müller";
    let r = client.salutation(name, &options).await.unwrap();

    assert_eq!(r.salutation.formal, "Sehr geehrte Frau Dr. Müller,");
    assert_eq!(r.salutation.informal, "Liebe Anna,");
    assert_eq!(r.salutation.neutral, "Guten Tag Dr. Anna Müller,");
    assert_eq!(r.form, "gendered");
    assert_eq!(r.reason, None);
    assert_eq!(r.parts.academic.as_deref(), Some("Dr."));
    assert_eq!(r.probability, Some(99));
    assert_eq!(r.gender_source.as_deref(), Some("lookup"));
    assert_eq!(r.country_source.as_deref(), Some("country"));
    assert_eq!(r.credits_remaining, 4999);
}

#[tokio::test]
async fn salutation_by_parts_returns_the_neutral_form() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 1, "credits_remaining": 98, "data_version": "2026.10", "request_id": "req_2",
        "country_source": null, "query": "Ahmet Yılmaz", "language": "tr", "form": "neutral",
        "reason": "gender_neutral_requested",
        "salutation": { "formal": "Sayın Ahmet Yılmaz,", "informal": "Merhaba Ahmet,", "neutral": "Sayın Ahmet Yılmaz," },
        "parts": { "opening": "Sayın", "courtesy": null, "academic": null, "name": "Ahmet Yılmaz" },
        "gender": null, "gender_source": null, "probability": null, "confidence": null,
        "first_name": "Ahmet", "last_name": "Yılmaz", "name_type": "personal", "country": null
    });

    Mock::given(method("POST"))
        .and(path("/salutation"))
        .and(body_json(json!({ "first_name": "Ahmet", "last_name": "Yılmaz", "language": "tr", "gender": "neutral" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = SalutationOptions {
        language: Some("tr".into()),
        gender: Some("neutral".into()),
        ..Default::default()
    };
    let r = client
        .salutation_by_parts("Ahmet", "Yılmaz", &options)
        .await
        .unwrap();

    assert_eq!(r.form, "neutral");
    assert_eq!(r.reason.as_deref(), Some("gender_neutral_requested"));
    assert_eq!(r.salutation.neutral, "Sayın Ahmet Yılmaz,");
    assert_eq!(r.parts.courtesy, None);
    assert_eq!(r.parts.academic, None);
    assert_eq!(r.gender, None);
    assert_eq!(r.probability, None);
    assert_eq!(r.country_source, None);
}

#[tokio::test]
async fn salutation_bulk_keeps_the_order() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 3, "credits_remaining": 96, "data_version": "2026.10", "request_id": "req_3",
        "took_ms": 4, "country_source": null, "language": "de",
        "summary": { "total": 3, "gendered": 1, "neutral": 1, "organization": 1 },
        "results": [
            {
                "query": "Dr. Anna Müller", "language": "de", "form": "gendered", "reason": null,
                "salutation": { "formal": "Sehr geehrte Frau Dr. Müller,", "informal": "Liebe Anna,", "neutral": "Guten Tag Dr. Anna Müller," },
                "parts": { "opening": "Sehr geehrte", "courtesy": "Frau", "academic": "Dr.", "name": "Müller" },
                "gender": "female", "gender_source": "lookup", "probability": 99, "confidence": "high",
                "first_name": "Anna", "last_name": "Müller", "name_type": "personal", "country": null
            },
            {
                "query": "Kim Meyer", "language": "de", "form": "neutral", "reason": "below_min_probability",
                "salutation": { "formal": "Guten Tag Kim Meyer,", "informal": "Hallo Kim,", "neutral": "Guten Tag Kim Meyer," },
                "parts": { "opening": "Guten Tag", "courtesy": null, "academic": null, "name": "Kim Meyer" },
                "gender": null, "gender_source": null, "probability": null, "confidence": null,
                "first_name": "Kim", "last_name": "Meyer", "name_type": "personal", "country": null
            },
            {
                "query": "Acme GmbH", "language": "de", "form": "organization", "reason": null,
                "salutation": { "formal": "Sehr geehrte Damen und Herren,", "informal": "Sehr geehrte Damen und Herren,", "neutral": "Sehr geehrte Damen und Herren," },
                "parts": { "opening": "Sehr geehrte Damen und Herren", "courtesy": null, "academic": null, "name": null },
                "gender": null, "gender_source": null, "probability": null, "confidence": null,
                "first_name": null, "last_name": null, "name_type": "organization", "country": null
            }
        ]
    });

    Mock::given(method("POST"))
        .and(path("/salutation/bulk"))
        .and(body_json(
            json!({ "names": ["Dr. Anna Müller", "Kim Meyer", "Acme GmbH"], "language": "de" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = SalutationOptions {
        language: Some("de".into()),
        ..Default::default()
    };
    let names = ["Dr. Anna Müller", "Kim Meyer", "Acme GmbH"];
    let r = client.salutation_bulk(&names, &options).await.unwrap();

    assert_eq!(r.results.len(), 3);
    for (result, name) in r.results.iter().zip(names) {
        assert_eq!(result.query, name);
    }
    assert_eq!(
        r.results[1].reason.as_deref(),
        Some("below_min_probability")
    );
    assert_eq!(r.results[2].form, "organization");
    assert_eq!(r.results[2].parts.name, None);
    assert_eq!(r.summary.total, 3);
    assert_eq!(r.summary.organization, 1);
    assert_eq!(r.language, "de");
    assert_eq!(r.credits_charged, 3);
}

#[tokio::test]
async fn an_unsupported_salutation_language_is_an_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/salutation"))
        .respond_with(ResponseTemplate::new(422).set_body_json(json!({
            "error": "invalid_input", "message": "Unsupported language.", "field": "language",
            "supported": ["en", "de", "tr"], "request_id": "req_4"
        })))
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = SalutationOptions {
        language: Some("xx".into()),
        ..Default::default()
    };
    match client.salutation("Anna", &options).await.unwrap_err() {
        Error::Api { status, error, .. } => {
            assert_eq!(status, 422);
            assert_eq!(error, "invalid_input");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn name_check_parses_signals_and_evidence() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 1, "credits_remaining": 4999, "data_version": "2026.10",
        "request_id": "req_5", "country_source": "country", "query": "asdf qwerty",
        "assessment": "implausible", "score": 0,
        "signals": [
            {
                "code": "keyboard_pattern", "severity": "high",
                "part": "first_name", "value": "asdf"
            },
            {
                "code": "keyboard_pattern", "severity": "high",
                "part": "last_name", "value": "qwerty"
            },
            { "code": "first_name_not_found", "severity": "medium", "part": null, "value": null }
        ],
        "first_name": "Asdf", "last_name": "Qwerty", "name_type": "personal",
        "evidence": { "first_name_status": "not_found", "first_name_counted_records": 0 }
    });
    let expected = json!({ "name": "asdf qwerty", "country": "US" });

    Mock::given(method("POST"))
        .and(path("/name-check"))
        .and(header("authorization", "Bearer ng_live_test"))
        .and(body_json(expected))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = NameCheckOptions {
        country: Some("US".into()),
        ..Default::default()
    };
    let r = client.name_check("asdf qwerty", &options).await.unwrap();

    assert_eq!(r.assessment, "implausible");
    assert_eq!(r.score, 0);
    assert_eq!(r.signals.len(), 3);
    assert_eq!(r.signals[0].code, "keyboard_pattern");
    assert_eq!(r.signals[0].severity, "high");
    assert_eq!(r.signals[0].part.as_deref(), Some("first_name"));
    assert_eq!(r.signals[0].value.as_deref(), Some("asdf"));
    assert_eq!(r.signals[2].code, "first_name_not_found");
    assert_eq!(r.signals[2].part, None);
    assert_eq!(r.signals[2].value, None);
    assert_eq!(r.first_name.as_deref(), Some("Asdf"));
    assert_eq!(r.last_name.as_deref(), Some("Qwerty"));
    assert_eq!(r.name_type, "personal");
    let status = r.evidence.first_name_status.as_deref();
    assert_eq!(status, Some("not_found"));
    assert_eq!(r.evidence.first_name_counted_records, 0);
    assert_eq!(r.country_source.as_deref(), Some("country"));
    assert_eq!(r.request_id.as_deref(), Some("req_5"));
    assert_eq!(r.credits_remaining, 4999);
}

#[tokio::test]
async fn name_check_by_parts_sends_only_set_fields() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 1, "credits_remaining": 98, "data_version": "2026.10",
        "request_id": "req_6", "country_source": null, "query": "Jennifer Null",
        "assessment": "plausible", "score": 96,
        "signals": [
            {
                "code": "first_name_attested", "severity": "positive",
                "part": "first_name", "value": "Jennifer"
            }
        ],
        "first_name": "Jennifer", "last_name": "Null", "name_type": "personal",
        "evidence": { "first_name_status": null, "first_name_counted_records": 5871000 }
    });
    let expected = json!({
        "first_name": "Jennifer", "last_name": "Null", "locale": "en-US"
    });

    Mock::given(method("POST"))
        .and(path("/name-check"))
        .and(body_json(expected))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = NameCheckOptions {
        locale: Some("en-US".into()),
        ..Default::default()
    };
    let r = client
        .name_check_by_parts("Jennifer", "Null", &options)
        .await
        .unwrap();

    assert_eq!(r.assessment, "plausible");
    assert_eq!(r.score, 96);
    assert_eq!(r.signals[0].severity, "positive");
    assert_eq!(r.evidence.first_name_status, None);
    assert_eq!(r.evidence.first_name_counted_records, 5871000);
    assert_eq!(r.country_source, None);
}

#[tokio::test]
async fn name_check_bulk_keeps_the_order() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 3, "credits_remaining": 96, "data_version": "2026.10",
        "request_id": "req_7", "took_ms": 4, "country_source": "ip",
        "summary": { "total": 3, "plausible": 1, "suspicious": 1, "implausible": 1 },
        "results": [
            {
                "query": "Jennifer Null", "assessment": "plausible", "score": 96, "signals": [],
                "first_name": "Jennifer", "last_name": "Null", "name_type": "personal",
                "evidence": {
                    "first_name_status": "counted", "first_name_counted_records": 5871000
                }
            },
            {
                "query": "Mickey Mouse", "assessment": "suspicious", "score": 35,
                "signals": [
                    {
                        "code": "fictional_character", "severity": "medium",
                        "part": "full", "value": "Mickey Mouse"
                    }
                ],
                "first_name": "Mickey", "last_name": "Mouse", "name_type": "personal",
                "evidence": { "first_name_status": "counted", "first_name_counted_records": 1200 }
            },
            {
                "query": "asdf qwerty", "assessment": "implausible", "score": 0,
                "signals": [
                    {
                        "code": "keyboard_pattern", "severity": "high",
                        "part": "first_name", "value": "asdf"
                    }
                ],
                "first_name": "Asdf", "last_name": "Qwerty", "name_type": "personal",
                "evidence": { "first_name_status": "not_found", "first_name_counted_records": 0 }
            }
        ]
    });
    let expected = json!({
        "names": ["Jennifer Null", "Mickey Mouse", "asdf qwerty"], "ip": "203.0.113.7"
    });

    Mock::given(method("POST"))
        .and(path("/name-check/bulk"))
        .and(body_json(expected))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = NameCheckOptions {
        ip: Some("203.0.113.7".into()),
        ..Default::default()
    };
    let names = ["Jennifer Null", "Mickey Mouse", "asdf qwerty"];
    let r = client.name_check_bulk(&names, &options).await.unwrap();

    assert_eq!(r.results.len(), 3);
    for (result, name) in r.results.iter().zip(names) {
        assert_eq!(result.query, name);
    }
    assert_eq!(r.results[0].assessment, "plausible");
    assert_eq!(r.results[1].assessment, "suspicious");
    assert_eq!(r.results[2].assessment, "implausible");
    assert_eq!(r.results[1].signals[0].code, "fictional_character");
    assert_eq!(r.results[1].signals[0].part.as_deref(), Some("full"));
    assert_eq!(r.summary.total, 3);
    assert_eq!(r.summary.plausible, 1);
    assert_eq!(r.summary.suspicious, 1);
    assert_eq!(r.summary.implausible, 1);
    assert_eq!(r.country_source.as_deref(), Some("ip"));
    assert_eq!(r.took_ms, 4);
    assert_eq!(r.credits_charged, 3);
}

#[tokio::test]
async fn a_name_check_without_credits_is_an_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/name-check"))
        .and(body_json(json!({ "name": "Anna" })))
        .respond_with(ResponseTemplate::new(402).set_body_json(json!({
            "error": "no_credits", "message": "No credits left.", "request_id": "req_8"
        })))
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = NameCheckOptions::default();
    match client.name_check("Anna", &options).await.unwrap_err() {
        Error::Api {
            status,
            error,
            request_id,
            ..
        } => {
            assert_eq!(status, 402);
            assert_eq!(error, "no_credits");
            assert_eq!(request_id.as_deref(), Some("req_8"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn age_parses_both_ranges() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 1, "credits_remaining": 49999, "request_id": "req_1",
        "name": "Brittany", "first_name": "Brittany", "gender": null,
        "age": 36,
        "age_range": { "low": 32, "high": 38 },
        "age_range_80": { "low": 28, "high": 41 },
        "birth_year": 1990, "sample_size": 353775, "births": 361434,
        "country": "US", "country_source": "default",
        "source": "ssa", "series": "1880-2024", "reference_year": 2026,
        "reason": null
    });

    Mock::given(method("POST"))
        .and(path("/age"))
        .and(header("authorization", "Bearer ng_live_test"))
        .and(body_json(json!({ "name": "Brittany" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = AgeOptions::default();
    let r = client.age("Brittany", &options).await.unwrap();

    assert_eq!(r.name, "Brittany");
    assert_eq!(r.first_name.as_deref(), Some("Brittany"));
    assert_eq!(r.gender, None);
    assert_eq!(r.age, Some(36));
    let range = r.age_range.as_ref().unwrap();
    assert_eq!((range.low, range.high), (32, 38));
    let range_80 = r.age_range_80.as_ref().unwrap();
    assert_eq!((range_80.low, range_80.high), (28, 41));
    assert_eq!(r.birth_year, Some(1990));
    assert_eq!(r.sample_size, 353775);
    assert_eq!(r.births, 361434);
    assert_eq!(r.country, "US");
    assert_eq!(r.country_source, "default");
    assert_eq!(r.source.as_deref(), Some("ssa"));
    assert_eq!(r.series.as_deref(), Some("1880-2024"));
    assert_eq!(r.reference_year, 2026);
    assert_eq!(r.reason, None);
    assert_eq!(r.credits_charged, 1);
    assert_eq!(r.credits_remaining, 49999);
    assert_eq!(r.request_id.as_deref(), Some("req_1"));
}

#[tokio::test]
async fn age_sends_only_set_options() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 1, "credits_remaining": 49998, "request_id": "req_2",
        "name": "Camille", "first_name": "Camille", "gender": "female",
        "age": 24,
        "age_range": { "low": 19, "high": 31 },
        "age_range_80": { "low": 14, "high": 38 },
        "birth_year": 2002, "sample_size": 120000, "births": 125000,
        "country": "FR", "country_source": "country",
        "source": "insee", "series": "1900-2024", "reference_year": 2026,
        "reason": null
    });
    let expected = json!({
        "name": "Camille", "gender": "female", "country": "FR", "ip": "203.0.113.7"
    });

    Mock::given(method("POST"))
        .and(path("/age"))
        .and(body_json(expected))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = AgeOptions {
        gender: Some("female".into()),
        country: Some("FR".into()),
        ip: Some("203.0.113.7".into()),
        ..Default::default()
    };
    let r = client.age("Camille", &options).await.unwrap();

    assert_eq!(r.gender.as_deref(), Some("female"));
    assert_eq!(r.age, Some(24));
    assert_eq!(r.country_source, "country");
}

#[tokio::test]
async fn age_country_not_covered_is_a_result_not_an_error() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 0, "credits_remaining": 49999, "request_id": "req_3",
        "name": "Ayşe", "first_name": "Ayşe", "gender": null,
        "age": null, "age_range": null, "age_range_80": null,
        "birth_year": null, "sample_size": 0, "births": 0,
        "country": "TR", "country_source": "country",
        "source": null, "series": null, "reference_year": 2026,
        "reason": "country_not_covered"
    });

    Mock::given(method("POST"))
        .and(path("/age"))
        .and(body_json(json!({ "name": "Ayşe", "country": "TR" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = AgeOptions {
        country: Some("TR".into()),
        ..Default::default()
    };
    let r = client.age("Ayşe", &options).await.unwrap();

    assert_eq!(r.age, None);
    assert!(r.age_range.is_none());
    assert!(r.age_range_80.is_none());
    assert_eq!(r.birth_year, None);
    assert_eq!(r.sample_size, 0);
    assert_eq!(r.source, None);
    assert_eq!(r.series, None);
    assert_eq!(r.reason.as_deref(), Some("country_not_covered"));
    assert_eq!(r.credits_charged, 0);
}

#[tokio::test]
async fn age_bulk_keeps_the_order() {
    let server = MockServer::start().await;
    let body = json!({
        "credits_charged": 2, "credits_remaining": 49997, "request_id": "req_4",
        "country_source": "default",
        "results": [
            {
                "name": "Brittany", "first_name": "Brittany", "gender": "female",
                "age": 36,
                "age_range": { "low": 32, "high": 38 },
                "age_range_80": { "low": 28, "high": 41 },
                "birth_year": 1990, "sample_size": 353775, "births": 361434,
                "country": "US", "country_source": "default",
                "source": "ssa", "series": "1880-2024", "reference_year": 2026,
                "reason": null
            },
            {
                "name": "Xqzt", "first_name": "Xqzt", "gender": "female",
                "age": null, "age_range": null, "age_range_80": null,
                "birth_year": null, "sample_size": 0, "births": 0,
                "country": "US", "country_source": "default",
                "source": "ssa", "series": "1880-2024", "reference_year": 2026,
                "reason": "not_found"
            }
        ]
    });
    let expected = json!({ "names": ["Brittany", "Xqzt"], "gender": "female" });

    Mock::given(method("POST"))
        .and(path("/age/bulk"))
        .and(body_json(expected))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = NameGender::new("ng_live_test").with_base_url(server.uri());
    let options = AgeOptions {
        gender: Some("female".into()),
        ..Default::default()
    };
    let names = ["Brittany", "Xqzt"];
    let r = client.age_bulk(&names, &options).await.unwrap();

    assert_eq!(r.results.len(), 2);
    for (result, name) in r.results.iter().zip(names) {
        assert_eq!(result.name, name);
    }
    assert_eq!(r.results[0].age, Some(36));
    let range = r.results[0].age_range.as_ref().unwrap();
    assert_eq!((range.low, range.high), (32, 38));
    assert_eq!(r.results[1].age, None);
    let reason = r.results[1].reason.as_deref();
    assert_eq!(reason, Some("not_found"));
    assert_eq!(r.country_source.as_deref(), Some("default"));
    assert_eq!(r.credits_charged, 2);
    assert_eq!(r.request_id.as_deref(), Some("req_4"));
}

#[tokio::test]
#[ignore = "needs NAMEGENDER_LIVE_KEY and network"]
async fn live_api() {
    let key = std::env::var("NAMEGENDER_LIVE_KEY").expect("NAMEGENDER_LIVE_KEY");
    let r = NameGender::new(key)
        .name("Emma", &Options::default())
        .await
        .unwrap();
    assert_eq!(r.gender.as_deref(), Some("female"));
}
