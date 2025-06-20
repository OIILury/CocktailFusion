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
pub struct InlineResponse503 {
    /// Errors contains a list of errors that caused the not ready status.
    #[serde(rename = "errors")]
    pub errors: ::std::collections::HashMap<String, String>,
}

impl InlineResponse503 {
    pub fn new(errors: ::std::collections::HashMap<String, String>) -> InlineResponse503 {
        InlineResponse503 { errors }
    }
}
