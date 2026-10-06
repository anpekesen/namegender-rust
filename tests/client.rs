// The salutation bodies are large json! literals.
#![recursion_limit = "256"]

use namegender::{Error, NameGender, Options, SalutationOptions, ValueType};
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
#[ignore = "needs NAMEGENDER_LIVE_KEY and network"]
async fn live_api() {
    let key = std::env::var("NAMEGENDER_LIVE_KEY").expect("NAMEGENDER_LIVE_KEY");
    let r = NameGender::new(key)
        .name("Emma", &Options::default())
        .await
        .unwrap();
    assert_eq!(r.gender.as_deref(), Some("female"));
}
