# NameGender Rust

```toml
[dependencies]
namegender = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use namegender::{NameGender, Options};

#[tokio::main]
async fn main() -> Result<(), namegender::Error> {
    let client = NameGender::new(std::env::var("NAMEGENDER_API_KEY").unwrap());

    let r = client
        .name("Andrea", &Options { country: Some("IT".into()), ..Default::default() })
        .await?;
    println!("{:?} {}% from {} people", r.gender, r.probability, r.sample_size);
    Ok(())
}
```

An async client built on `reqwest` with rustls; no OpenSSL needed. Get an API
key from the [namegender.com](https://namegender.com) dashboard and keep it on
the server.

## Lookups

```rust
use namegender::{Options, ValueType};

client.name("Ayşe Yılmaz", &Options::default()).await?;
client.email("jane.doe@example.com", &Options::default()).await?;
client.username("jane_doe_92", &Options::default()).await?;

// Up to 100 values per request, results in the same order
let bulk = client.bulk(&["Emma", "Liam", "Andrea"], ValueType::Name, &Options::default()).await?;
for r in &bulk.results {
    println!("{} {:?}", r.query, r.gender);
}
```

Every value costs one credit, unknown results included. `gender` is `None`
when the API does not know; check `probability` and `sample_size` before
trusting an answer.

## Options

| Field | Meaning |
|---|---|
| `country` | Two-letter country code. Andrea is male in Italy and female in Germany. |
| `locale` | Language tag such as `it-IT`; its region is the country when `country` is absent. `en` sets none. |
| `ip` | End user IP address; its country is used when neither of the above applies. Not stored. |
| `best_guess` | Return the more likely gender even when the evidence is weak. |
| `ai_fallback` | Ask a language model when the dataset has no answer. Requires AI lookups to be enabled on the account. |

`country_source` on a response says which one set the country: `"country"`,
`"locale"`, `"ip"` or `None`.

## Errors

A non-2xx response becomes `Error::Api` with the HTTP status and the API's
reason code. Branch on the code, not the message:

```rust
match client.name("Emma", &Options::default()).await {
    Err(namegender::Error::Api { error, .. }) if error == "no_credits" => { /* top up */ }
    Err(e) => return Err(e),
    Ok(r) => println!("{:?}", r.gender),
}
```

## Country distribution and account

```rust
let dist = client.countries("Mehmet", 10).await?;
println!("{}", dist.basis["note"]);

let account = client.account().await?; // costs nothing
println!("{} credits left", account.credits_remaining);
```

`countries` reports which countries a name is recorded in. It is not a
country-of-origin or ethnicity inference: `registrations` is counted volume
from the countries that publish counted birth statistics, and `attested_in` is
presence with no weight attached. Show `basis["note"]` next to any percentage.

## License

MIT
