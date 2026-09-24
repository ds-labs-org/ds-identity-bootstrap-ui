//! List / add / activate / rotate / revoke a participant context's keys
//! (`IdentityHubClient::{list,get,add,activate,rotate,revoke}_keypair`).
//!
//! Not part of the 3-step bootstrap wizard itself (the wizard's inline key,
//! created alongside the participant context, needs no separate activation
//! step of its own) -- this is the standalone key-management surface for
//! everything that comes *after* bootstrap: rotating a key before it
//! expires, adding a second key for a different `KeyPairUsage`, revoking a
//! compromised one.

use edc_identity_hub_client::models::{KeyDescriptor, KeyPairResource};
use edc_identity_hub_client::{IdentityHubClient, IdentityHubClientError, IdentityHubClientVersion};
use patternfly_yew::prelude::*;
use yew::platform::spawn_local;
use yew::prelude::*;

/// Builds the `PUT .../keypairs` request body from validated form input.
/// Pure and synchronous -- covered by plain `cargo test` below.
///
/// Only the "generate a new key" path is exposed here (mirrors the
/// bootstrap wizard's own inline-key step); providing an existing JWK/PEM
/// is real API surface (`KeyDescriptor::public_key_jwk` /
/// `public_key_pem`) but not a lifecycle panel concern -- add it if/when a
/// consumer needs it.
pub fn build_key_descriptor(key_id: &str, algorithm: &str) -> Result<KeyDescriptor, String> {
    let key_id = key_id.trim();
    if key_id.is_empty() {
        return Err("Key ID is required.".to_string());
    }

    let algorithm = algorithm.trim();
    if algorithm.is_empty() {
        return Err("Algorithm is required.".to_string());
    }

    Ok(KeyDescriptor {
        key_id: Some(key_id.to_string()),
        key_generator_params: Some(serde_json::json!({ "algorithm": algorithm })),
        ..Default::default()
    })
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

fn state_label(state: i32) -> &'static str {
    match state {
        100 => "Created",
        200 => "Activated",
        300 => "Rotated",
        400 => "Revoked",
        _ => "Unknown",
    }
}

#[derive(Clone, PartialEq)]
enum LoadState {
    Loading,
    Loaded(Vec<KeyPairResource>),
    Error(String),
}

#[derive(Properties, PartialEq)]
pub struct KeypairLifecyclePanelProps {
    /// The page's own origin, e.g. `https://issuer-admin.ds-labs.org`.
    pub endpoint: String,
    #[prop_or_default]
    pub bearer_token: Option<String>,
    pub participant_context_id: String,
}

#[function_component(KeypairLifecyclePanel)]
pub fn keypair_lifecycle_panel(props: &KeypairLifecyclePanelProps) -> Html {
    let load_state = use_state(|| LoadState::Loading);
    let reload = use_state(|| 0u32);
    let action_error = use_state(|| Option::<String>::None);
    let show_add_modal = use_state(|| false);
    let form_key_id = use_state(String::new);
    let form_algorithm = use_state(|| "EC".to_string());
    let submitting = use_state(|| false);

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
                    match client.list_keypairs(&participant_context_id).await {
                        Ok(keypairs) => load_state.set(LoadState::Loaded(keypairs)),
                        Err(err) => load_state.set(LoadState::Error(describe_error(err))),
                    }
                });
                || ()
            },
        );
    }

    let bump_reload = {
        let reload = reload.clone();
        Callback::from(move |()| reload.set(*reload + 1))
    };

    let open_add_modal = {
        let show_add_modal = show_add_modal.clone();
        let form_key_id = form_key_id.clone();
        let action_error = action_error.clone();
        Callback::from(move |_: MouseEvent| {
            form_key_id.set(String::new());
            action_error.set(None);
            show_add_modal.set(true);
        })
    };

    let close_add_modal = {
        let show_add_modal = show_add_modal.clone();
        Callback::from(move |()| show_add_modal.set(false))
    };

    let on_submit_new_keypair = {
        let form_key_id = form_key_id.clone();
        let form_algorithm = form_algorithm.clone();
        let action_error = action_error.clone();
        let submitting = submitting.clone();
        let show_add_modal = show_add_modal.clone();
        let bump_reload = bump_reload.clone();
        let endpoint = props.endpoint.clone();
        let bearer_token = props.bearer_token.clone();
        let participant_context_id = props.participant_context_id.clone();

        Callback::from(move |_: MouseEvent| {
            let descriptor = match build_key_descriptor(&form_key_id, &form_algorithm) {
                Ok(descriptor) => descriptor,
                Err(message) => {
                    action_error.set(Some(message));
                    return;
                }
            };

            action_error.set(None);
            submitting.set(true);

            let endpoint = endpoint.clone();
            let bearer_token = bearer_token.clone();
            let participant_context_id = participant_context_id.clone();
            let action_error = action_error.clone();
            let submitting = submitting.clone();
            let show_add_modal = show_add_modal.clone();
            let bump_reload = bump_reload.clone();

            spawn_local(async move {
                let client = build_client(&endpoint, bearer_token);
                match client
                    .add_keypair(&participant_context_id, &descriptor, false)
                    .await
                {
                    Ok(()) => {
                        submitting.set(false);
                        show_add_modal.set(false);
                        bump_reload.emit(());
                    }
                    Err(err) => {
                        action_error.set(Some(describe_error(err)));
                        submitting.set(false);
                    }
                }
            });
        })
    };

    let on_activate = {
        let action_error = action_error.clone();
        let bump_reload = bump_reload.clone();
        let endpoint = props.endpoint.clone();
        let bearer_token = props.bearer_token.clone();
        let participant_context_id = props.participant_context_id.clone();
        Callback::from(move |key_pair_id: String| {
            let action_error = action_error.clone();
            let bump_reload = bump_reload.clone();
            let endpoint = endpoint.clone();
            let bearer_token = bearer_token.clone();
            let participant_context_id = participant_context_id.clone();
            spawn_local(async move {
                let client = build_client(&endpoint, bearer_token);
                match client
                    .activate_keypair(&participant_context_id, &key_pair_id)
                    .await
                {
                    Ok(()) => bump_reload.emit(()),
                    Err(err) => action_error.set(Some(describe_error(err))),
                }
            });
        })
    };

    let on_rotate = {
        let action_error = action_error.clone();
        let bump_reload = bump_reload.clone();
        let endpoint = props.endpoint.clone();
        let bearer_token = props.bearer_token.clone();
        let participant_context_id = props.participant_context_id.clone();
        Callback::from(move |key_pair_id: String| {
            let action_error = action_error.clone();
            let bump_reload = bump_reload.clone();
            let endpoint = endpoint.clone();
            let bearer_token = bearer_token.clone();
            let participant_context_id = participant_context_id.clone();
            spawn_local(async move {
                let client = build_client(&endpoint, bearer_token);
                // No replacement key supplied here (`None`) -- the server
                // generates one using the same algorithm as the key being
                // rotated. `duration=0` retires the old key immediately
                // rather than keeping it valid for a grace period.
                match client
                    .rotate_keypair(&participant_context_id, &key_pair_id, None, 0)
                    .await
                {
                    Ok(()) => bump_reload.emit(()),
                    Err(err) => action_error.set(Some(describe_error(err))),
                }
            });
        })
    };

    let on_revoke = {
        let action_error = action_error.clone();
        let bump_reload = bump_reload.clone();
        let endpoint = props.endpoint.clone();
        let bearer_token = props.bearer_token.clone();
        let participant_context_id = props.participant_context_id.clone();
        Callback::from(move |key_pair_id: String| {
            let confirmed = web_sys::window()
                .and_then(|window| {
                    window
                        .confirm_with_message(&format!(
                            "Revoke keypair \"{key_pair_id}\"? Anything it signed becomes untrustworthy."
                        ))
                        .ok()
                })
                .unwrap_or(true);
            if !confirmed {
                return;
            }

            let action_error = action_error.clone();
            let bump_reload = bump_reload.clone();
            let endpoint = endpoint.clone();
            let bearer_token = bearer_token.clone();
            let participant_context_id = participant_context_id.clone();
            spawn_local(async move {
                let client = build_client(&endpoint, bearer_token);
                match client
                    .revoke_keypair(&participant_context_id, &key_pair_id, None)
                    .await
                {
                    Ok(()) => bump_reload.emit(()),
                    Err(err) => action_error.set(Some(describe_error(err))),
                }
            });
        })
    };

    html! {
        <>
            if let Some(message) = &*action_error {
                <Alert r#type={AlertType::Danger} title="Keypair action failed" inline=true>
                    <p>{ message.clone() }</p>
                </Alert>
            }

            <Toolbar>
                <ToolbarContent>
                    <ToolbarItem>
                        <Button
                            label="Add keypair"
                            icon={Icon::Plus}
                            variant={ButtonVariant::Primary}
                            onclick={open_add_modal}
                        />
                    </ToolbarItem>
                </ToolbarContent>
            </Toolbar>

            {
                match &*load_state {
                    LoadState::Loading => html!(
                        <Bullseye><Spinner aria_label="Loading keypairs" /></Bullseye>
                    ),
                    LoadState::Error(message) => html!(
                        <Alert r#type={AlertType::Danger} title="Could not load keypairs" inline=true>
                            <p>{ message.clone() }</p>
                        </Alert>
                    ),
                    LoadState::Loaded(keypairs) if keypairs.is_empty() => html!(
                        <EmptyState title="No keypairs yet" icon={Icon::Key}>
                            <p>{ "Add a keypair to sign credentials, presentations, or tokens with." }</p>
                        </EmptyState>
                    ),
                    LoadState::Loaded(keypairs) => html!(
                        <KeypairTable
                            keypairs={keypairs.clone()}
                            on_activate={on_activate}
                            on_rotate={on_rotate}
                            on_revoke={on_revoke}
                        />
                    ),
                }
            }

            if *show_add_modal {
                <Modal
                    title="Add keypair"
                    variant={ModalVariant::Medium}
                    onclose={close_add_modal.clone()}
                    footer={html!(
                        <>
                            <Button
                                label={ if *submitting { "Adding..." } else { "Add" } }
                                variant={ButtonVariant::Primary}
                                disabled={*submitting}
                                onclick={on_submit_new_keypair}
                            />
                            <Button
                                label="Cancel"
                                variant={ButtonVariant::Link}
                                disabled={*submitting}
                                onclick={close_add_modal.reform(|_: MouseEvent| ())}
                            />
                        </>
                    )}
                >
                    <Form>
                        <FormGroup label="Key ID" required=true>
                            <TextInput
                                value={(*form_key_id).clone()}
                                required=true
                                placeholder="did.ds-labs.org#key-2"
                                onchange={{
                                    let form_key_id = form_key_id.clone();
                                    Callback::from(move |value: String| form_key_id.set(value))
                                }}
                            />
                        </FormGroup>
                        <FormGroup label="Algorithm" required=true>
                            <TextInput
                                value={(*form_algorithm).clone()}
                                required=true
                                onchange={{
                                    let form_algorithm = form_algorithm.clone();
                                    Callback::from(move |value: String| form_algorithm.set(value))
                                }}
                            />
                        </FormGroup>
                    </Form>
                </Modal>
            }
        </>
    }
}

#[derive(Properties, PartialEq)]
struct KeypairTableProps {
    keypairs: Vec<KeyPairResource>,
    on_activate: Callback<String>,
    on_rotate: Callback<String>,
    on_revoke: Callback<String>,
}

#[function_component(KeypairTable)]
fn keypair_table(props: &KeypairTableProps) -> Html {
    html! {
        <ComposableTable>
            <thead>
                <tr class="pf-v6-c-table__tr">
                    <th class="pf-v6-c-table__th" scope="col">{ "Key ID" }</th>
                    <th class="pf-v6-c-table__th" scope="col">{ "State" }</th>
                    <th class="pf-v6-c-table__th" scope="col">{ "Default" }</th>
                    <th class="pf-v6-c-table__th" scope="col">{ "" }</th>
                </tr>
            </thead>
            <TableBody>
                { for props.keypairs.iter().map(|keypair| {
                    let id = keypair.id.clone();
                    let activate_id = id.clone();
                    let rotate_id = id.clone();
                    let revoke_id = id.clone();
                    let on_activate = props.on_activate.clone();
                    let on_rotate = props.on_rotate.clone();
                    let on_revoke = props.on_revoke.clone();
                    html! {
                        <TableRow key={id.clone()}>
                            <TableData>{ keypair.key_id.clone() }</TableData>
                            <TableData>{ state_label(keypair.state) }</TableData>
                            <TableData>{ if keypair.default_pair { "yes" } else { "no" } }</TableData>
                            <TableData action=true>
                                <Button
                                    label="Activate"
                                    variant={ButtonVariant::Secondary}
                                    onclick={Callback::from(move |_: MouseEvent| on_activate.emit(activate_id.clone()))}
                                />
                                { " " }
                                <Button
                                    label="Rotate"
                                    variant={ButtonVariant::Secondary}
                                    onclick={Callback::from(move |_: MouseEvent| on_rotate.emit(rotate_id.clone()))}
                                />
                                { " " }
                                <Button
                                    label="Revoke"
                                    variant={ButtonVariant::DangerSecondary}
                                    onclick={Callback::from(move |_: MouseEvent| on_revoke.emit(revoke_id.clone()))}
                                />
                            </TableData>
                        </TableRow>
                    }
                }) }
            </TableBody>
        </ComposableTable>
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn rejects_a_blank_key_id() {
        let result = build_key_descriptor("  ", "EC");
        assert_eq!(result.unwrap_err(), "Key ID is required.");
    }

    #[test]
    fn rejects_a_blank_algorithm() {
        let result = build_key_descriptor("did.example.com#key-2", "  ");
        assert_eq!(result.unwrap_err(), "Algorithm is required.");
    }

    #[test]
    fn builds_a_generator_descriptor_for_valid_input() {
        let descriptor =
            build_key_descriptor("did.example.com#key-2", "EC").expect("valid input should build a KeyDescriptor");

        assert_eq!(descriptor.key_id.as_deref(), Some("did.example.com#key-2"));
        assert_eq!(
            descriptor.key_generator_params,
            Some(serde_json::json!({ "algorithm": "EC" }))
        );
        assert!(descriptor.public_key_jwk.is_none());
        assert!(descriptor.public_key_pem.is_none());
    }

    #[test]
    fn state_label_maps_known_codes() {
        assert_eq!(state_label(100), "Created");
        assert_eq!(state_label(200), "Activated");
        assert_eq!(state_label(300), "Rotated");
        assert_eq!(state_label(400), "Revoked");
        assert_eq!(state_label(999), "Unknown");
    }
}
