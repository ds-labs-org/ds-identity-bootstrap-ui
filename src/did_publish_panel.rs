//! Publish / unpublish / query a participant context's DID, and show
//! whether it is currently published (`IdentityHubClient::{publish,
//! unpublish,query,get_did_state}_did`).
//!
//! This is step 3 of the bootstrap wizard -- "the go-live moment" per the
//! reviewed mockup -- and also usable standalone for later unpublish/
//! republish management.

use edc_identity_hub_client::{IdentityHubClient, IdentityHubClientError, IdentityHubClientVersion};
use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;

/// The DID a `did:web` participant resolves to, derived from the
/// `participantId` chosen when its context was created.
///
/// EDC's `ParticipantContext` request body sets its own `did` field to the
/// bare `participantId` string (confirmed in the real model -- see
/// `participant_context_panel::build_participant_context`'s doc comment),
/// while every `identity-api` DID-management endpoint
/// (`publish`/`unpublish`/`query`/`state`) instead wants the full method
/// string. Re-deriving it here, rather than threading a second ID through
/// every prop in this crate, keeps "the participant ID" the only identifier
/// a consumer has to hold onto end-to-end.
pub fn did_from_participant_id(participant_id: &str) -> String {
    format!("did:web:{participant_id}")
}

/// Whether an `IdentityHubClient::get_did_state` result means "live and
/// resolvable" -- checked case-insensitively since the real
/// `DidManagementApiController` answers with the bare `DidState` enum name
/// (`PUBLISHED`), and this crate's own client already normalizes away
/// surrounding quotes but not casing.
pub fn is_published(state: &str) -> bool {
    state.trim().eq_ignore_ascii_case("published")
}

fn build_client(endpoint: &str) -> IdentityHubClient {
    IdentityHubClient::new(
        reqwest::Client::new(),
        endpoint.to_string(),
        None,
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

#[derive(Clone, PartialEq)]
enum StateLoad {
    Loading,
    Loaded(String),
    Error(String),
}

#[derive(Properties, PartialEq)]
pub struct DidPublishPanelProps {
    /// The page's own origin, e.g. `https://issuer-admin.ds-labs.org`.
    pub endpoint: String,
    pub participant_context_id: String,
    /// The `participantId` this context was created with -- combined with
    /// `did_from_participant_id` to get the DID string every DID-management
    /// call needs.
    pub participant_id: String,
    /// Fired once, the moment the DID is observed to be published -- either
    /// because `publish_did` just succeeded, or because the initial
    /// `get_did_state` load already found it `PUBLISHED` (re-entering the
    /// wizard on an already-bootstrapped context).
    #[prop_or_default]
    pub on_published: Callback<()>,
}

#[function_component(DidPublishPanel)]
pub fn did_publish_panel(props: &DidPublishPanelProps) -> Html {
    let did = did_from_participant_id(&props.participant_id);
    let state_load = use_state(|| StateLoad::Loading);
    let reload = use_state(|| 0u32);
    let acting = use_state(|| false);
    let action_error = use_state(|| Option::<String>::None);

    {
        let state_load = state_load.clone();
        let endpoint = props.endpoint.clone();
        let participant_context_id = props.participant_context_id.clone();
        let did = did.clone();
        use_effect_with((participant_context_id.clone(), did.clone(), *reload), move |_| {
            state_load.set(StateLoad::Loading);
            let state_load = state_load.clone();
            let endpoint = endpoint.clone();
            let participant_context_id = participant_context_id.clone();
            let did = did.clone();
            spawn_local(async move {
                let client = build_client(&endpoint);
                match client.get_did_state(&participant_context_id, &did).await {
                    Ok(state) => state_load.set(StateLoad::Loaded(state)),
                    Err(err) => state_load.set(StateLoad::Error(describe_error(err))),
                }
            });
            || ()
        });
    }

    // Auto-advance the wizard the moment the DID is observed as published --
    // whether that's because this panel just published it, or because it
    // already was (re-entering the wizard on a finished bootstrap).
    {
        let on_published = props.on_published.clone();
        let published = matches!(&*state_load, StateLoad::Loaded(state) if is_published(state));
        use_effect_with(published, move |published| {
            if *published {
                on_published.emit(());
            }
            || ()
        });
    }

    let do_publish = {
        let acting = acting.clone();
        let action_error = action_error.clone();
        let reload = reload.clone();
        let endpoint = props.endpoint.clone();
        let participant_context_id = props.participant_context_id.clone();
        let did = did.clone();
        Callback::from(move |_: MouseEvent| {
            acting.set(true);
            action_error.set(None);
            let acting = acting.clone();
            let action_error = action_error.clone();
            let reload = reload.clone();
            let current_reload = *reload;
            let endpoint = endpoint.clone();
            let participant_context_id = participant_context_id.clone();
            let did = did.clone();
            spawn_local(async move {
                let client = build_client(&endpoint);
                match client.publish_did(&participant_context_id, &did).await {
                    Ok(()) => {
                        acting.set(false);
                        reload.set(current_reload + 1);
                    }
                    Err(err) => {
                        action_error.set(Some(describe_error(err)));
                        acting.set(false);
                    }
                }
            });
        })
    };

    let do_unpublish = {
        let acting = acting.clone();
        let action_error = action_error.clone();
        let reload = reload.clone();
        let endpoint = props.endpoint.clone();
        let participant_context_id = props.participant_context_id.clone();
        let did = did.clone();
        Callback::from(move |_: MouseEvent| {
            acting.set(true);
            action_error.set(None);
            let acting = acting.clone();
            let action_error = action_error.clone();
            let reload = reload.clone();
            let current_reload = *reload;
            let endpoint = endpoint.clone();
            let participant_context_id = participant_context_id.clone();
            let did = did.clone();
            spawn_local(async move {
                let client = build_client(&endpoint);
                match client.unpublish_did(&participant_context_id, &did).await {
                    Ok(()) => {
                        acting.set(false);
                        reload.set(current_reload + 1);
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
                <Alert r#type={AlertType::Danger} title="DID action failed" inline=true>
                    <p>{ message.clone() }</p>
                </Alert>
            }

            <p><strong>{ did.clone() }</strong></p>

            {
                match &*state_load {
                    StateLoad::Loading => html!(
                        <Bullseye><Spinner aria_label="Checking DID state" /></Bullseye>
                    ),
                    StateLoad::Error(message) => html!(
                        <Alert r#type={AlertType::Danger} title="Could not check DID state" inline=true>
                            <p>{ message.clone() }</p>
                        </Alert>
                    ),
                    StateLoad::Loaded(state) => {
                        let published = is_published(state);
                        // A single `Button` slot whose label/variant/handler
                        // depend on `published`, rather than two `<Button/>`
                        // arms inside `ActionGroup`'s `if`/`else` -- that
                        // shape doesn't type-check against `ActionGroup`'s
                        // `ChildrenWithProps<Button>` (confirmed against the
                        // real compiler error, not assumed).
                        let (action_label, action_variant, action_onclick) = if published {
                            ("Unpublish", ButtonVariant::DangerSecondary, do_unpublish)
                        } else {
                            ("Publish DID", ButtonVariant::Primary, do_publish)
                        };
                        html! {
                            <>
                                <Label
                                    label={ state.clone() }
                                    color={ if published { Color::Green } else { Color::Grey } }
                                />
                                <p class="pf-v6-u-color-200">
                                    if published {
                                        { "Publicly resolvable now." }
                                    } else {
                                        { "Publishing makes this DID publicly resolvable. This cannot be meaningfully undone once other parties have cached it." }
                                    }
                                </p>
                                <ActionGroup>
                                    <Button
                                        label={action_label}
                                        variant={action_variant}
                                        disabled={*acting}
                                        loading={*acting}
                                        onclick={action_onclick}
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
    fn derives_the_did_web_string_from_a_plain_host() {
        assert_eq!(did_from_participant_id("did.ds-labs.org"), "did:web:did.ds-labs.org");
    }

    #[test]
    fn derives_the_did_web_string_for_a_path_style_participant_id() {
        assert_eq!(
            did_from_participant_id("example.com:user:alice"),
            "did:web:example.com:user:alice"
        );
    }

    #[test]
    fn is_published_matches_the_real_state_name_case_insensitively() {
        assert!(is_published("PUBLISHED"));
        assert!(is_published("published"));
        assert!(is_published("  PUBLISHED  "));
    }

    #[test]
    fn is_published_rejects_every_other_state() {
        assert!(!is_published("UNPUBLISHED"));
        assert!(!is_published("NOT_PUBLISHED"));
        assert!(!is_published(""));
    }
}
