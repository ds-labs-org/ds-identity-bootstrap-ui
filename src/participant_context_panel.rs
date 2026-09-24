//! Create / view / activate / deactivate / delete a single participant
//! context (`IdentityHubClient`'s `create_participant` / `get_participant` /
//! `activate_participant` / `delete_participant`).
//!
//! Two modes, chosen by `participant_context_id`:
//! - `None` -- shows the create form (step 1 of the bootstrap wizard).
//! - `Some(id)` -- shows the existing context's state plus
//!   activate/deactivate/delete actions (step 2, and standalone management).
//!
//! `IdentityHubClient` builds every URL as `{endpoint}/api/identity/{version}/...`
//! (the `/api/identity` segment is hardcoded inside the client itself, not a
//! caller-supplied path) -- so `endpoint` here is just the page's own origin,
//! same as every other client this workspace builds.

use edc_identity_hub_client::models::{IdentityService, IdentityServiceType, ParticipantContext};
use edc_identity_hub_client::{IdentityHubClient, IdentityHubClientError, IdentityHubClientVersion};
use patternfly_yew::prelude::*;
use uuid::Uuid;
use yew::platform::spawn_local;
use yew::prelude::*;

/// The role a participant context is being bootstrapped for. Only affects
/// the default `roles` list -- EDC's own `ParticipantContext` domain object
/// has no notion of "role" beyond the free-form `roles: Vec<String>` this
/// maps onto.
///
/// `Display`/`FromStr` (round-tripping through the same lowercase strings
/// that become the request body's `roles` entry) are what `FormSelect<K>`
/// requires of its generic parameter -- see `patternfly_yew::FormSelect`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParticipantRole {
    #[default]
    Authority,
    Connector,
}

impl ParticipantRole {
    fn role_label(self) -> &'static str {
        match self {
            ParticipantRole::Authority => "authority",
            ParticipantRole::Connector => "connector",
        }
    }
}

impl std::fmt::Display for ParticipantRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.role_label())
    }
}

impl std::str::FromStr for ParticipantRole {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "authority" => Ok(ParticipantRole::Authority),
            "connector" => Ok(ParticipantRole::Connector),
            _ => Err(()),
        }
    }
}

/// Builds the `POST .../participants` request body from validated form
/// input -- the exact shape the identity-api expects (mirrors the mockup:
/// `participantId`, `roles`, `serviceEndpoints`, an inline generated EC
/// key). Pure and synchronous so it's covered by plain `cargo test` below,
/// with no yew/wasm-bindgen dependency needed.
///
/// The participant context ID is deliberately the same string as the
/// participant ID: EDC's `DidManagementApiController` passes
/// `participantContextId` straight through to its authorization check
/// without decoding it (confirmed against the real v0.18.0 source), so any
/// caller-chosen string works so long as it's used consistently across
/// every call for this context -- and the create/activate/publish forms
/// this crate exposes never need two different IDs to keep straight.
pub fn build_participant_context(
    participant_id: &str,
    role: ParticipantRole,
    service_endpoint_type: &str,
    service_endpoint_url: &str,
) -> Result<ParticipantContext, String> {
    let participant_id = participant_id.trim();
    if participant_id.is_empty() {
        return Err("Participant ID is required.".to_string());
    }

    let service_endpoint_type = service_endpoint_type.trim();
    if service_endpoint_type.is_empty() {
        return Err("Service endpoint type is required.".to_string());
    }

    let service_endpoint_url = service_endpoint_url.trim();
    if service_endpoint_url.is_empty() {
        return Err("Service endpoint URL is required.".to_string());
    }
    if !(service_endpoint_url.starts_with("https://") || service_endpoint_url.starts_with("http://")) {
        return Err("Service endpoint URL must be an absolute http(s) URL.".to_string());
    }

    let service_endpoint = IdentityService {
        id: Uuid::new_v4().to_string(),
        r#type: service_endpoint_type
            .parse::<IdentityServiceType>()
            .unwrap_or(IdentityServiceType::Custom(service_endpoint_type.to_string())),
        service_endpoint: service_endpoint_url.to_string(),
    };

    Ok(ParticipantContext::new(
        participant_id.to_string(),
        participant_id.to_string(),
        false,
        vec![service_endpoint],
        vec![role.role_label().to_string()],
        None,
    ))
}

fn build_client(endpoint: &str, bearer_token: Option<String>) -> IdentityHubClient {
    IdentityHubClient::new(
        reqwest::Client::new(),
        endpoint.to_string(),
        bearer_token,
        IdentityHubClientVersion::V1Beta,
    )
}

fn describe_error(error: IdentityHubClientError) -> String {
    match error {
        IdentityHubClientError::Reqwest(error) => error.to_string(),
        IdentityHubClientError::Response(response) => {
            format!("identity-api responded with {}", response.status())
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct ParticipantContextPanelProps {
    /// The page's own origin, e.g. `https://issuer-admin.ds-labs.org`.
    pub endpoint: String,
    #[prop_or_default]
    pub bearer_token: Option<String>,
    /// `None` renders the create form; `Some(id)` renders the
    /// view/activate/deactivate/delete panel for that existing context.
    #[prop_or_default]
    pub participant_context_id: Option<String>,
    /// Fired with the new context's ID once `create_participant` succeeds.
    #[prop_or_default]
    pub on_created: Callback<String>,
    /// Fired once `activate_participant(.., true)` succeeds.
    #[prop_or_default]
    pub on_activated: Callback<()>,
    /// Fired once `delete_participant` succeeds.
    #[prop_or_default]
    pub on_deleted: Callback<()>,
}

#[function_component(ParticipantContextPanel)]
pub fn participant_context_panel(props: &ParticipantContextPanelProps) -> Html {
    match &props.participant_context_id {
        None => html!(<CreatePanel ..CreatePanelProps::from(props) />),
        Some(id) => html!(
            <ManagePanel
                endpoint={props.endpoint.clone()}
                bearer_token={props.bearer_token.clone()}
                participant_context_id={id.clone()}
                on_activated={props.on_activated.clone()}
                on_deleted={props.on_deleted.clone()}
            />
        ),
    }
}

#[derive(Properties, PartialEq)]
struct CreatePanelProps {
    endpoint: String,
    bearer_token: Option<String>,
    on_created: Callback<String>,
}

impl From<&ParticipantContextPanelProps> for CreatePanelProps {
    fn from(props: &ParticipantContextPanelProps) -> Self {
        Self {
            endpoint: props.endpoint.clone(),
            bearer_token: props.bearer_token.clone(),
            on_created: props.on_created.clone(),
        }
    }
}

#[function_component(CreatePanel)]
fn create_panel(props: &CreatePanelProps) -> Html {
    let participant_id = use_state(String::new);
    let role = use_state(ParticipantRole::default);
    let service_endpoint_type = use_state(|| "CredentialService".to_string());
    let service_endpoint_url = use_state(String::new);
    let error = use_state(|| Option::<String>::None);
    let submitting = use_state(|| false);

    let onsubmit = {
        let participant_id = participant_id.clone();
        let role = role.clone();
        let service_endpoint_type = service_endpoint_type.clone();
        let service_endpoint_url = service_endpoint_url.clone();
        let error = error.clone();
        let submitting = submitting.clone();
        let endpoint = props.endpoint.clone();
        let bearer_token = props.bearer_token.clone();
        let on_created = props.on_created.clone();

        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();

            let body = match build_participant_context(
                &participant_id,
                *role,
                &service_endpoint_type,
                &service_endpoint_url,
            ) {
                Ok(body) => body,
                Err(message) => {
                    error.set(Some(message));
                    return;
                }
            };

            error.set(None);
            submitting.set(true);

            let created_id = (*participant_id).clone();
            let endpoint = endpoint.clone();
            let bearer_token = bearer_token.clone();
            let on_created = on_created.clone();
            let error = error.clone();
            let submitting = submitting.clone();

            spawn_local(async move {
                let client = build_client(&endpoint, bearer_token);
                match client.create_participant(&body).await {
                    Ok(_response) => {
                        submitting.set(false);
                        on_created.emit(created_id);
                    }
                    Err(err) => {
                        error.set(Some(describe_error(err)));
                        submitting.set(false);
                    }
                }
            });
        })
    };

    html! {
        <Form {onsubmit}>
            if let Some(message) = &*error {
                <Alert r#type={AlertType::Danger} title="Could not create participant context" inline=true>
                    <p>{ message.clone() }</p>
                </Alert>
            }
            <FormGroup
                label="Participant ID"
                required=true
                helper_text={FormHelperText::from(format!(
                    "Resolves as did:web:{}",
                    if participant_id.is_empty() { "…".to_string() } else { (*participant_id).clone() }
                ).as_str())}
            >
                <TextInput
                    required=true
                    placeholder="did.ds-labs.org"
                    value={(*participant_id).clone()}
                    onchange={{
                        let participant_id = participant_id.clone();
                        move |value: String| participant_id.set(value)
                    }}
                />
            </FormGroup>
            <FormGroup label="Role on this hub" required=true>
                <FormSelect<ParticipantRole>
                    value={Some(*role)}
                    onchange={{
                        let role = role.clone();
                        move |value: Option<ParticipantRole>| {
                            if let Some(value) = value {
                                role.set(value);
                            }
                        }
                    }}
                >
                    <FormSelectOption<ParticipantRole> value={ParticipantRole::Authority} description="Authority (issuer)" />
                    <FormSelectOption<ParticipantRole> value={ParticipantRole::Connector} description="Connector (participant)" />
                </FormSelect<ParticipantRole>>
            </FormGroup>
            <FormGroup label="Service endpoint type" required=true>
                <TextInput
                    required=true
                    value={(*service_endpoint_type).clone()}
                    onchange={{
                        let service_endpoint_type = service_endpoint_type.clone();
                        move |value: String| service_endpoint_type.set(value)
                    }}
                />
            </FormGroup>
            <FormGroup label="Service endpoint URL" required=true>
                <TextInput
                    required=true
                    placeholder="https://did.ds-labs.org/api/sts"
                    value={(*service_endpoint_url).clone()}
                    onchange={{
                        let service_endpoint_url = service_endpoint_url.clone();
                        move |value: String| service_endpoint_url.set(value)
                    }}
                />
            </FormGroup>
            <FormGroup label="Initial signing key">
                <p class="pf-v6-u-color-200">{ "Generated inline -- EC (P-256)." }</p>
            </FormGroup>
            <ActionGroup>
                <Button
                    r#type={ButtonType::Submit}
                    label={ if *submitting { "Creating..." } else { "Create context" } }
                    variant={ButtonVariant::Primary}
                    disabled={*submitting}
                    loading={*submitting}
                />
            </ActionGroup>
        </Form>
    }
}

// Not `PartialEq`: `edc_identity_hub_client::models::Participant` doesn't
// implement it, and nothing here ever needs to compare two `LoadState`s for
// equality (only match on which variant it is).
#[derive(Clone)]
enum LoadState {
    Loading,
    Loaded(edc_identity_hub_client::models::Participant),
    Error(String),
}

#[derive(Properties, PartialEq)]
struct ManagePanelProps {
    endpoint: String,
    bearer_token: Option<String>,
    participant_context_id: String,
    on_activated: Callback<()>,
    on_deleted: Callback<()>,
}

#[function_component(ManagePanel)]
fn manage_panel(props: &ManagePanelProps) -> Html {
    let load_state = use_state(|| LoadState::Loading);
    let reload = use_state(|| 0u32);
    let action_error = use_state(|| Option::<String>::None);
    let acting = use_state(|| false);

    {
        let load_state = load_state.clone();
        let endpoint = props.endpoint.clone();
        let bearer_token = props.bearer_token.clone();
        let participant_context_id = props.participant_context_id.clone();
        use_effect_with(
            (participant_context_id.clone(), *reload),
            move |_| {
                load_state.set(LoadState::Loading);
                let load_state = load_state.clone();
                let endpoint = endpoint.clone();
                let bearer_token = bearer_token.clone();
                let participant_context_id = participant_context_id.clone();
                spawn_local(async move {
                    let client = build_client(&endpoint, bearer_token);
                    match client.get_participant(&participant_context_id).await {
                        Ok(participant) => load_state.set(LoadState::Loaded(participant)),
                        Err(err) => load_state.set(LoadState::Error(describe_error(err))),
                    }
                });
                || ()
            },
        );
    }

    let do_activate = {
        let acting = acting.clone();
        let action_error = action_error.clone();
        let reload = reload.clone();
        let endpoint = props.endpoint.clone();
        let bearer_token = props.bearer_token.clone();
        let participant_context_id = props.participant_context_id.clone();
        let on_activated = props.on_activated.clone();
        Callback::from(move |is_active: bool| {
            acting.set(true);
            action_error.set(None);
            let acting = acting.clone();
            let action_error = action_error.clone();
            let reload = reload.clone();
            let current_reload = *reload;
            let endpoint = endpoint.clone();
            let bearer_token = bearer_token.clone();
            let participant_context_id = participant_context_id.clone();
            let on_activated = on_activated.clone();
            spawn_local(async move {
                let client = build_client(&endpoint, bearer_token);
                match client
                    .activate_participant(&participant_context_id, is_active)
                    .await
                {
                    Ok(()) => {
                        acting.set(false);
                        reload.set(current_reload + 1);
                        if is_active {
                            on_activated.emit(());
                        }
                    }
                    Err(err) => {
                        action_error.set(Some(describe_error(err)));
                        acting.set(false);
                    }
                }
            });
        })
    };

    let do_delete = {
        let acting = acting.clone();
        let action_error = action_error.clone();
        let endpoint = props.endpoint.clone();
        let bearer_token = props.bearer_token.clone();
        let participant_context_id = props.participant_context_id.clone();
        let on_deleted = props.on_deleted.clone();
        Callback::from(move |_: MouseEvent| {
            acting.set(true);
            action_error.set(None);
            let acting = acting.clone();
            let action_error = action_error.clone();
            let endpoint = endpoint.clone();
            let bearer_token = bearer_token.clone();
            let participant_context_id = participant_context_id.clone();
            let on_deleted = on_deleted.clone();
            spawn_local(async move {
                let client = build_client(&endpoint, bearer_token);
                match client.delete_participant(&participant_context_id).await {
                    Ok(_) => {
                        acting.set(false);
                        on_deleted.emit(());
                    }
                    Err(err) => {
                        action_error.set(Some(describe_error(err)));
                        acting.set(false);
                    }
                }
            });
        })
    };

    html! {
        <>
            if let Some(message) = &*action_error {
                <Alert r#type={AlertType::Danger} title="Action failed" inline=true>
                    <p>{ message.clone() }</p>
                </Alert>
            }
            {
                match &*load_state {
                    LoadState::Loading => html!(
                        <Bullseye><Spinner aria_label="Loading participant context" /></Bullseye>
                    ),
                    LoadState::Error(message) => html!(
                        <Alert r#type={AlertType::Danger} title="Could not load participant context" inline=true>
                            <p>{ message.clone() }</p>
                        </Alert>
                    ),
                    LoadState::Loaded(participant) => {
                        let activate_onclick = do_activate.reform(|_: MouseEvent| true);
                        let deactivate_onclick = do_activate.reform(|_: MouseEvent| false);
                        html! {
                            <>
                                <p><strong>{ participant.participant_context_id.clone() }</strong></p>
                                <p class="pf-v6-u-color-200">
                                    { format!("did: {} · state code: {}", participant.did.url(), participant.state) }
                                </p>
                                <ActionGroup>
                                    <Button
                                        label="Activate"
                                        variant={ButtonVariant::Primary}
                                        disabled={*acting}
                                        onclick={activate_onclick}
                                    />
                                    <Button
                                        label="Deactivate"
                                        variant={ButtonVariant::Secondary}
                                        disabled={*acting}
                                        onclick={deactivate_onclick}
                                    />
                                    <Button
                                        label="Delete"
                                        variant={ButtonVariant::DangerSecondary}
                                        disabled={*acting}
                                        onclick={do_delete}
                                    />
                                </ActionGroup>
                            </>
                        }
                    }
                }
            }
        </>
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn rejects_a_blank_participant_id() {
        let result = build_participant_context("  ", ParticipantRole::Authority, "CredentialService", "https://example.com");
        assert_eq!(result.unwrap_err(), "Participant ID is required.");
    }

    #[test]
    fn rejects_a_blank_service_endpoint_type() {
        let result = build_participant_context("did.example.com", ParticipantRole::Authority, "  ", "https://example.com");
        assert_eq!(result.unwrap_err(), "Service endpoint type is required.");
    }

    #[test]
    fn rejects_a_blank_service_endpoint_url() {
        let result = build_participant_context("did.example.com", ParticipantRole::Authority, "CredentialService", "  ");
        assert_eq!(result.unwrap_err(), "Service endpoint URL is required.");
    }

    #[test]
    fn rejects_a_non_absolute_service_endpoint_url() {
        let result = build_participant_context(
            "did.example.com",
            ParticipantRole::Authority,
            "CredentialService",
            "example.com/sts",
        );
        assert_eq!(
            result.unwrap_err(),
            "Service endpoint URL must be an absolute http(s) URL."
        );
    }

    #[test]
    fn builds_the_exact_shape_the_mockup_shows_for_a_valid_authority_submission() {
        let context = build_participant_context(
            "did.ds-labs.org",
            ParticipantRole::Authority,
            "CredentialService",
            "https://did.ds-labs.org/api/sts",
        )
        .expect("valid input should build a ParticipantContext");

        let json = serde_json::to_value(&context).expect("ParticipantContext should serialize");
        assert_eq!(json["participantId"], "did.ds-labs.org");
        assert_eq!(json["participantContextId"], "did.ds-labs.org");
        assert_eq!(json["active"], false);
        assert_eq!(json["roles"], serde_json::json!(["authority"]));
        assert_eq!(
            json["serviceEndpoints"][0]["serviceEndpoint"],
            "https://did.ds-labs.org/api/sts"
        );
        assert_eq!(json["serviceEndpoints"][0]["type"], "CredentialService");
        assert_eq!(json["key"]["keyGeneratorParams"]["algorithm"], "EC");
        assert_eq!(json["key"]["keyId"], "did.ds-labs.org#key-1");
    }

    #[test]
    fn connector_role_serializes_as_connector() {
        let context = build_participant_context(
            "connector-1.example.com",
            ParticipantRole::Connector,
            "CredentialService",
            "https://connector-1.example.com/sts",
        )
        .expect("valid input should build a ParticipantContext");

        let json = serde_json::to_value(&context).expect("ParticipantContext should serialize");
        assert_eq!(json["roles"], serde_json::json!(["connector"]));
    }
}
