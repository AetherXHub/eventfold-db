//! JWT authentication interceptor for EventfoldDB.
//!
//! This module provides [`JwtInterceptor`], a [`tonic::service::Interceptor`]
//! implementation that validates HS256 JSON Web Tokens on incoming gRPC requests.
//! When wired into the tonic server, every request must carry a valid
//! `authorization: Bearer <token>` metadata header or it is rejected with
//! `UNAUTHENTICATED` before reaching service logic.

/// Holds the decoded signing key and validation config for HS256 JWT verification.
///
/// Implements [`tonic::service::Interceptor`] so it can be plugged directly into
/// a tonic `InterceptedService` wrapper. The struct is `Clone` as required by
/// tonic for per-connection cloning.
///
/// # Examples
///
/// ```
/// use eventfold_db::auth::JwtInterceptor;
///
/// let interceptor = JwtInterceptor::new("my-secret");
/// ```
#[derive(Clone)]
pub struct JwtInterceptor {
    decoding_key: jsonwebtoken::DecodingKey,
    validation: jsonwebtoken::Validation,
}

impl JwtInterceptor {
    /// Construct a new `JwtInterceptor` from an HS256 shared secret.
    ///
    /// The secret is used to build a [`jsonwebtoken::DecodingKey`] and a
    /// [`jsonwebtoken::Validation`] configured for HS256 with `exp` validation
    /// enabled. Both `exp` and `sub` are required claims.
    ///
    /// # Arguments
    ///
    /// * `secret` - The shared HS256 signing secret as a UTF-8 string.
    pub fn new(secret: &str) -> Self {
        let decoding_key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.validate_exp = true;
        validation.leeway = 0;
        validation.required_spec_claims = ["exp", "sub"].iter().map(|s| (*s).to_string()).collect();
        Self {
            decoding_key,
            validation,
        }
    }
}

/// Internal claims struct used for JWT token decoding.
///
/// Only `sub` (subject) and `exp` (expiration) are extracted; the store does
/// not act on the subject value beyond requiring its presence.
#[derive(serde::Deserialize)]
struct Claims {
    /// Subject claim -- identifies the token holder.
    #[allow(dead_code)]
    sub: String,
    /// Expiration timestamp (seconds since Unix epoch).
    #[allow(dead_code)]
    exp: u64,
}

impl tonic::service::Interceptor for JwtInterceptor {
    fn call(&mut self, request: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        // Step 1: Read the `authorization` metadata key.
        let token_str = match request.metadata().get("authorization") {
            Some(value) => match value.to_str() {
                Ok(s) => s.to_owned(),
                Err(_) => {
                    tracing::debug!("rejected: non-ASCII authorization header");
                    return Err(tonic::Status::unauthenticated(
                        "missing authorization header",
                    ));
                }
            },
            None => {
                tracing::debug!("rejected: missing authorization header");
                return Err(tonic::Status::unauthenticated(
                    "missing authorization header",
                ));
            }
        };

        // Step 2: Strip the case-sensitive "Bearer " prefix.
        let token = match token_str.strip_prefix("Bearer ") {
            Some(t) => t,
            None => {
                tracing::debug!("rejected: invalid authorization header format");
                return Err(tonic::Status::unauthenticated(
                    "invalid authorization header format",
                ));
            }
        };

        // Step 3: Decode and validate the JWT.
        match jsonwebtoken::decode::<Claims>(token, &self.decoding_key, &self.validation) {
            Ok(_) => Ok(request),
            Err(e) => {
                tracing::debug!("rejected: {e}");
                Err(tonic::Status::unauthenticated(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::service::Interceptor;

    /// Serializable claims for minting test tokens.
    #[derive(serde::Serialize)]
    struct TestClaims {
        sub: String,
        exp: u64,
    }

    /// Helper: encode a token with the given secret and claims.
    fn encode_token(secret: &str, sub: &str, exp: u64) -> String {
        let claims = TestClaims {
            sub: sub.to_owned(),
            exp,
        };
        jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("encoding should not fail in tests")
    }

    /// Helper: return seconds since Unix epoch.
    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before Unix epoch")
            .as_secs()
    }

    #[test]
    fn missing_header_returns_unauthenticated() {
        let mut interceptor = JwtInterceptor::new("secret");
        let request = tonic::Request::new(());
        let result = interceptor.call(request);
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
        assert!(
            status.message().contains("missing"),
            "expected 'missing' in: {}",
            status.message()
        );
    }

    #[test]
    fn missing_bearer_prefix_returns_unauthenticated() {
        let mut interceptor = JwtInterceptor::new("secret");
        let token = encode_token("secret", "user1", now_secs() + 3600);
        // Omit the "Bearer " prefix -- just send the raw token.
        let mut request = tonic::Request::new(());
        request
            .metadata_mut()
            .insert("authorization", token.parse().expect("valid ASCII"));
        let result = interceptor.call(request);
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
        assert!(
            status.message().contains("format"),
            "expected 'format' in: {}",
            status.message()
        );
    }

    #[test]
    fn valid_token_returns_ok() {
        let mut interceptor = JwtInterceptor::new("secret");
        let token = encode_token("secret", "user1", now_secs() + 3600);
        let mut request = tonic::Request::new(());
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {token}").parse().expect("valid ASCII"),
        );
        let result = interceptor.call(request);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn wrong_secret_returns_unauthenticated() {
        // Token signed with "wrong-secret", interceptor configured with "correct-secret".
        let mut interceptor = JwtInterceptor::new("correct-secret");
        let token = encode_token("wrong-secret", "user1", now_secs() + 3600);
        let mut request = tonic::Request::new(());
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {token}").parse().expect("valid ASCII"),
        );
        let result = interceptor.call(request);
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn expired_token_returns_unauthenticated() {
        let mut interceptor = JwtInterceptor::new("secret");
        // exp set to 1 second in the past.
        let token = encode_token("secret", "user1", now_secs() - 1);
        let mut request = tonic::Request::new(());
        request.metadata_mut().insert(
            "authorization",
            format!("Bearer {token}").parse().expect("valid ASCII"),
        );
        let result = interceptor.call(request);
        let status = result.unwrap_err();
        assert_eq!(status.code(), tonic::Code::Unauthenticated);
    }
}
