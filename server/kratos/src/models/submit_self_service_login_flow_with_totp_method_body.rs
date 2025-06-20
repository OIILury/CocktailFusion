/*
 * Ory Kratos API
 *
 * Documentation for all public and administrative Ory Kratos APIs. Public and administrative APIs are exposed on different ports. Public APIs can face the public internet without any protection while administrative APIs should never be exposed without prior authorization. To protect the administative API port you should use something like Nginx, Ory Oathkeeper, or any other technology capable of authorizing incoming requests.
 *
 * The version of the OpenAPI document: v0.10.1
 * Contact: hi@ory.sh
 * Generated by: https://openapi-generator.tech
 */

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmitSelfServiceLoginFlowWithTotpMethodBody {
    /// Sending the anti-csrf token is only required for browser login flows.
    #[serde(rename = "csrf_token", skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
    /// Method should be set to \"totp\" when logging in using the TOTP strategy.
    #[serde(rename = "method")]
    pub method: String,
    /// The TOTP code.
    #[serde(rename = "totp_code")]
    pub totp_code: String,
}

impl SubmitSelfServiceLoginFlowWithTotpMethodBody {
    pub fn new(method: String, totp_code: String) -> SubmitSelfServiceLoginFlowWithTotpMethodBody {
        SubmitSelfServiceLoginFlowWithTotpMethodBody {
            csrf_token: None,
            method,
            totp_code,
        }
    }
}
