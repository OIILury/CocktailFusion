/*
 * Ory Kratos API
 *
 * Documentation for all public and administrative Ory Kratos APIs. Public and administrative APIs are exposed on different ports. Public APIs can face the public internet without any protection while administrative APIs should never be exposed without prior authorization. To protect the administative API port you should use something like Nginx, Ory Oathkeeper, or any other technology capable of authorizing incoming requests.
 *
 * The version of the OpenAPI document: v0.10.1
 * Contact: hi@ory.sh
 * Generated by: https://openapi-generator.tech
 */

/// SubmitSelfServiceSettingsFlowWithOidcMethodBody : nolint:deadcode,unused

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubmitSelfServiceSettingsFlowWithOidcMethodBody {
    /// Flow ID is the flow's ID.  in: query
    #[serde(rename = "flow", skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    /// Link this provider  Either this or `unlink` must be set.  type: string in: body
    #[serde(rename = "link", skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// Method  Should be set to profile when trying to update a profile.
    #[serde(rename = "method")]
    pub method: String,
    /// The identity's traits  in: body
    #[serde(rename = "traits", skip_serializing_if = "Option::is_none")]
    pub traits: Option<serde_json::Value>,
    /// Unlink this provider  Either this or `link` must be set.  type: string in: body
    #[serde(rename = "unlink", skip_serializing_if = "Option::is_none")]
    pub unlink: Option<String>,
}

impl SubmitSelfServiceSettingsFlowWithOidcMethodBody {
    /// nolint:deadcode,unused
    pub fn new(method: String) -> SubmitSelfServiceSettingsFlowWithOidcMethodBody {
        SubmitSelfServiceSettingsFlowWithOidcMethodBody {
            flow: None,
            link: None,
            method,
            traits: None,
            unlink: None,
        }
    }
}
