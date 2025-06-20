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
pub struct IdentityCredentialsOidcProvider {
    #[serde(
        rename = "initial_access_token",
        skip_serializing_if = "Option::is_none"
    )]
    pub initial_access_token: Option<String>,
    #[serde(rename = "initial_id_token", skip_serializing_if = "Option::is_none")]
    pub initial_id_token: Option<String>,
    #[serde(
        rename = "initial_refresh_token",
        skip_serializing_if = "Option::is_none"
    )]
    pub initial_refresh_token: Option<String>,
    #[serde(rename = "provider", skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(rename = "subject", skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

impl IdentityCredentialsOidcProvider {
    pub fn new() -> IdentityCredentialsOidcProvider {
        IdentityCredentialsOidcProvider {
            initial_access_token: None,
            initial_id_token: None,
            initial_refresh_token: None,
            provider: None,
            subject: None,
        }
    }
}
