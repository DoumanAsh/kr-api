use cucumber_rust as cucumber;
use cucumber::{WorldInit, async_trait};

//Copy of own lib, just because ring is such inconvenient dependency to have...
mod otpshka;

const BASE: &str = "https://api.kraken.com/0/";

#[inline]
fn get_nonce() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("System time is invalid").as_millis() as u64
}

#[derive(WorldInit)]
pub struct Context {
    http: reqwest::Client,
    method: reqwest::Method,
    path: &'static str,
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
        }
        _ => panic!("Unexpected input {}", input),
    }
}

#[cucumber::then(regex = "Get response")]
async fn get_response(ctx: &mut Context) {
    let req = ctx.http.request(ctx.method.clone(), format!("{}{}", BASE, ctx.path).as_str());

    let rsp = req.send().await.expect("Successful request");
    assert_eq!(rsp.status().as_u16(), 200);

    ctx.rsp = rsp.json().await.expect("To contain json");
}

#[cucumber::then(regex = "Validate format")]
async fn validate_response(ctx: &mut Context) {
    match ctx.rsp {
        serde_json::Value::Object(ref obj) => {
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
