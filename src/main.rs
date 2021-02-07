use serde::Serialize;
use cucumber_rust as cucumber;
use cucumber::{WorldInit, async_trait};

//Copy of own lib, just because ring is such inconvenient dependency to have...
mod otpshka;
mod sign;

const BASE: &str = "https://api.kraken.com/0/";

#[inline]
fn get_nonce() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("System time is invalid").as_millis() as u64
}

#[derive(Serialize)]
struct FormData {
    nonce: u64,
    otp: Option<String>,
}

#[derive(WorldInit)]
pub struct Context {
    http: reqwest::Client,
    method: reqwest::Method,
    path: &'static str,
    req_headers: reqwest::header::HeaderMap,
    req_payload: Option<FormData>,
    rsp: serde_json::Value,
}

#[async_trait(?Send)]
impl cucumber::World for Context {
    type Error = core::convert::Infallible;

    async fn new() -> Result<Self, Self::Error> {
        Ok(Self {
            http: reqwest::Client::new(),
            method: reqwest::Method::GET,
            path: "",
            req_headers: reqwest::header::HeaderMap::new(),
            req_payload: None,
            rsp: serde_json::Value::Null,
        })
    }
}

#[cucumber::given(regex = "I am checking (.*)")]
async fn given(ctx: &mut Context, input: String) {
    match input.as_str() {
        "server time" => {
            ctx.method = reqwest::Method::GET;
            ctx.path = "public/Time";
        },
        "XBT/USD" => {
            ctx.method = reqwest::Method::GET;
            ctx.path = "public/Ticker?pair=XXBTZUSD";
        },
        "open order" => {
            ctx.method = reqwest::Method::POST;
            ctx.path = "private/OpenOrders";
        },
        _ => panic!("Unexpected input {}", input),
    }
}

#[cucumber::given(regex = "Use API Auth")]
async fn api_key(ctx: &mut Context) {
    match std::env::var("KR_API_KEY") {
        Ok(key) => {
            let value = reqwest::header::HeaderValue::from_str(&key).expect("API Key must be valid to be set in header");
            ctx.req_headers.insert("API-Key", value);
        },
        Err(_) => panic!("API Key is not set to env 'KR_API_KEY'"),
    }

    match std::env::var("KR_API_OTP_TOKEN") {
        Ok(token) => {
            let mut buffer = Vec::new();
            buffer.resize(6, 0);

            let token = data_encoding::BASE32.decode(token.as_bytes()).expect("OTP token as base32 string");
            let totp = otpshka::TOTP::new(otpshka::Algorithm::SHA1, token);
            totp.generate_to_now(&mut buffer);
            ctx.req_payload = Some(FormData {
                nonce: get_nonce(),
                otp: Some(String::from_utf8(buffer).unwrap()),
            });
        },
        Err(_) => {
            ctx.req_payload = Some(FormData {
                nonce: get_nonce(),
                otp: None,
            });
        },
    }
}

#[cucumber::then(regex = "Get response")]
async fn get_response(ctx: &mut Context) {
    let req = ctx.http.request(ctx.method.clone(), format!("{}{}", BASE, ctx.path).as_str());

    let mut headers = Default::default();
    core::mem::swap(&mut ctx.req_headers, &mut headers);

    let req = if let Some(payload) = ctx.req_payload.take() {
        assert_eq!(ctx.method, reqwest::Method::POST);
        let key = match std::env::var("KR_API_SECRET") {
            Ok(secret) => data_encoding::BASE64.decode(secret.as_bytes()).expect("API Secret is not base64 string"),
            Err(_) => panic!("API secret is not set as 'KR_API_SECRET'"),
        };

        let mut req = req.headers(headers).form(&payload).build().expect("To build request");

        let sign = sign::generate(ctx.path, req.body().unwrap().as_bytes().unwrap(), payload.nonce, &key);
        let value = reqwest::header::HeaderValue::from_str(&sign).expect("API Signature must be valid to be set in header");
        req.headers_mut().insert("API-Sign", value);

        req
    } else {
        assert_eq!(ctx.method, reqwest::Method::GET);
        req.headers(headers).build().expect("To build request")
    };

    let rsp = ctx.http.execute(req).await.expect("Successful request");
    assert_eq!(rsp.status().as_u16(), 200);

    ctx.rsp = rsp.json().await.expect("To contain json");
}

#[cucumber::then(regex = "Validate format")]
async fn validate_response(ctx: &mut Context) {
    match ctx.rsp {
        serde_json::Value::Object(ref obj) => {
            assert_eq!(obj.len(), 2);
            assert!(obj.contains_key("error"));
            assert!(obj.contains_key("result"));
        },
        _ => panic!("API response must be an object"),
    }
}

#[cucumber::then(regex = "Check time")]
async fn check_time(ctx: &mut Context) {
    match ctx.rsp {
        serde_json::Value::Object(ref obj) => match obj.get("result").unwrap() {
            serde_json::Value::Object(ref obj) => {
                assert_eq!(obj.len(), 2);
                assert!(obj.get("unixtime").expect("Missing 'unixtime'").is_number());
                assert!(obj.get("rfc1123").expect("Missing 'rfc1123'").is_string());
            },
            _ => panic!("API result must be an object"),
        },
        _ => panic!("API response must be an object"),
    }
}

#[cucumber::then(regex = "Check order")]
async fn check_order(ctx: &mut Context) {
    match ctx.rsp {
        serde_json::Value::Object(ref obj) => match obj.get("result").unwrap() {
            serde_json::Value::Object(ref obj) => match obj.get("open").expect("'open' is not present") {
                serde_json::Value::Object(ref obj) => for (id, order) in obj {

                    //NOTE: we could probably add a elaborate verification
                    //      not sure how to approach it though
                    assert_ne!(id.len(), 0);
                    match order {
                        serde_json::Value::Object(ref order) => {
                            assert!(order.contains_key("refid"));
                            assert!(order.contains_key("status"));
                        },
                        _ => panic!("API OpenOrders's order must be an object"),
                    }
                },
                _ => panic!("API OpenOrders must be an object"),
            },
            _ => panic!("API result must be an object"),
        },
        _ => panic!("API response must be an object"),
    }
}

#[cucumber::then(regex = "Check XBT/USD")]
async fn check_currency_pair(ctx: &mut Context) {
    match ctx.rsp {
        serde_json::Value::Object(ref obj) => match obj.get("result").unwrap() {
            serde_json::Value::Object(ref obj) => match obj.get("XXBTZUSD").expect("Missing pair info") {
                serde_json::Value::Object(ref obj) => {
                    assert_eq!(obj.len(), 9);

                    assert!(obj.get("o").expect("missing 'o'").is_string());

                    let to_assert_strings = [
                        ("a", 3),
                        ("b", 3),
                        ("c", 2),
                        ("v", 2),
                        ("p", 2),
                        ("l", 2),
                        ("h", 2),
                    ];

                    let to_assert_nums = [
                        ("t", 2),
                    ];

                    for (field, num) in to_assert_strings.iter() {
                        match obj.get(*field) {
                            Some(serde_json::Value::Array(array)) => {
                                assert_eq!(array.len(), *num);
                                for elem in array {
                                    assert!(elem.is_string())
                                }
                            },
                            None => panic!("'{}' is missing", field),
                            _ => panic!("'{}' must be an array", field),
                        }
                    }

                    for (field, num) in to_assert_nums.iter() {
                        match obj.get(*field) {
                            Some(serde_json::Value::Array(array)) => {
                                assert_eq!(array.len(), *num);
                                for elem in array {
                                    assert!(elem.is_number())
                                }
                            },
                            None => panic!("'{}' is missing", field),
                            _ => panic!("'{}' must be an array", field),
                        }
                    }

                },
                _ => panic!("'XXBTZUSD must be an array"),
            },
            _ => panic!("API result must be an object"),
        },
        _ => panic!("API response must be an object"),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let runner = Context::init(&["./spec"]).enable_capture(false);
    runner.run_and_exit().await;
}
