#![allow(dead_code)]
use serde::Deserialize;
use serde_json::Value;
use std::fmt;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    AuthRequired(AuthRequired),
    AuthOk(AuthOk),
    AuthInvalid(AuthInvalid),
    Result(WSResult),
    Event(WSEvent),
}

/// Default response on HA WebSocket connection
#[derive(Debug, Deserialize)]
pub struct AuthRequired {
    pub ha_version: String,
}

/// Successful Authorization response
#[derive(Debug, Deserialize)]
pub struct AuthOk {
    pub ha_version: String,
}

/// Failed Authorization response (with error message)
#[derive(Debug, Deserialize)]
pub struct AuthInvalid {
    pub message: String,
}

/// HA WebSocket general response to a request
#[derive(Debug, Deserialize)]
pub struct WSResult {
    pub id: u64,
    pub success: bool,
    pub result: Option<Value>,
    pub error: Option<ErrorCode>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorCode {
    pub code: String,
    pub message: String,
}

///	HA WebSocket Event response
#[derive(Debug, Deserialize)]
pub struct WSEvent {
    pub id: u64,
    pub event: Value,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error {} -> {}", self.code, self.message)?;
        Ok(())
    }
}