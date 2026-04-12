use std::io::Result;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

pub fn value(message: &str) -> Value {
    serde_json::json!(message)
}

pub fn parse_value<T: DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

pub fn send<T: Serialize>(data: &T) {
    serde_json::to_string(data).unwrap();
}
