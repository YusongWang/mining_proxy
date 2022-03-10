use chrono::prelude::*;
use jsonwebtoken::{encode, EncodingKey, Header};

use serde::{Deserialize, Serialize};

use crate::JWT_SECRET;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub username: String,
    #[serde(with = "jwt_numeric_date")]
    exp: DateTime<Utc>,
}

impl Claims {
    pub fn new(username: String, exp: DateTime<Utc>) -> Self {
        let exp =
            exp.date()
                .and_hms_milli(exp.hour(), exp.minute(), exp.second(), 0);

        Self { username, exp }
    }
}

pub fn generate_jwt(claims: Claims) -> anyhow::Result<String> {
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )
    .map_err(|e| anyhow::anyhow!(e))
}

mod jwt_numeric_date {
    //! Custom serialization of DateTime<Utc> to conform with the JWT spec (RFC
    //! 7519 section 2, "Numeric Date")
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    /// Serializes a DateTime<Utc> to a Unix timestamp (milliseconds since
    /// 1970/1/1T00:00:00T)
    pub fn serialize<S>(
        date: &DateTime<Utc>, serializer: S,
    ) -> Result<S::Ok, S::Error>
    where S: Serializer {
        let timestamp = date.timestamp();
        serializer.serialize_i64(timestamp)
    }

    /// Attempts to deserialize an i64 and use as a Unix timestamp
    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<DateTime<Utc>, D::Error>
    where D: Deserializer<'de> {
        Utc.timestamp_opt(i64::deserialize(deserializer)?, 0)
            .single() // If there are multiple or no valid DateTimes from timestamp,
            // return None
            .ok_or_else(|| {
                serde::de::Error::custom("invalid Unix timestamp value")
            })
    }
}
