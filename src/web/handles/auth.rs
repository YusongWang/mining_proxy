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

    #[cfg(test)]
    mod tests {
        const EXPECTED_TOKEN: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJDdXN0b20gRGF0ZVRpbWUgc2VyL2RlIiwiaWF0IjowLCJleHAiOjMyNTAzNjgwMDAwfQ.RTgha0S53MjPC2pMA4e2oMzaBxSY3DMjiYR2qFfV55A";
        use super::super::Claims;
        use crate::JWT_SECRET;
        use chrono::{Duration, TimeZone, Utc};
        use jsonwebtoken::{
            decode, encode, DecodingKey, EncodingKey, Header, Validation,
        };

        #[test]
        fn round_trip() {
            let exp = Utc.timestamp(32503680000, 0);

            let claims = Claims::new("Hello world".into(), exp);

            let token = encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(JWT_SECRET.as_ref()),
            )
            .expect("Failed to encode claims");

            assert_eq!(&token, EXPECTED_TOKEN);

            let decoded = decode::<Claims>(
                &token,
                &DecodingKey::from_secret(JWT_SECRET.as_ref()),
                &Validation::default(),
            )
            .expect("Failed to decode token");

            assert_eq!(decoded.claims, claims);
        }
    }
}
