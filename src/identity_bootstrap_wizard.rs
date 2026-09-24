//! Composes the three panels into the real sequential bootstrap flow the
//! reviewed mockup describes: create the participant context (with its
//! inline key) -> activate -> publish the DID.
//!
//! The wizard tracks which step is current in its own state and advances
//! automatically on each step's success -- it never asks the user to click
//! "Next"; the panels' own success callbacks (`on_created`, `on_activated`,
//! `on_published`) drive `current_step` forward.

use crate::did_publish_panel::DidPublishPanel;
use crate::keypair_lifecycle_panel::KeypairLifecyclePanel;
use crate::participant_context_panel::ParticipantContextPanel;
use patternfly_yew::prelude::*;
use yew::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WizardStep {
    CreateContext,
    Activate,
    PublishDid,
    Done,
}

/// Which step the wizard opens on. Pure and synchronous (no network) --
/// covered by plain `cargo test` below, and what the wasm-bindgen-test
/// component tests assert on for "step-advancement behavior" without
/// depending on a live identity-api to answer any request.
///
/// A caller providing `participant_context_id` already has a context (per
/// this component's own doc comment: "re-entering the wizard on an
/// incomplete bootstrap") -- so step 1 is always skippable in that case.
/// Whether activation and publishing are *also* already done is instead
/// each panel's own concern once mounted: `ParticipantContextPanel`'s
/// Activate button is a harmless idempotent no-op against an
/// already-active context, and `DidPublishPanel` immediately fires
/// `on_published` (auto-advancing past this step too) the moment its own
/// `get_did_state` load finds the DID already `PUBLISHED` -- so a
/// consumer never needs to pre-compute the exact resume point here.
pub fn initial_step(participant_context_id: Option<&str>) -> WizardStep {
    match participant_context_id {
        Some(_) => WizardStep::Activate,
        None => WizardStep::CreateContext,
    }
}

fn step_status(step: WizardStep, current: WizardStep) -> ProgressStepperStepStatus {
    if step_index(step) < step_index(current) {
        ProgressStepperStepStatus::Success
    } else if step == current {
        ProgressStepperStepStatus::Info
    } else {
        ProgressStepperStepStatus::Pending
    }
}

fn step_index(step: WizardStep) -> u8 {
    match step {
        WizardStep::CreateContext => 0,
        WizardStep::Activate => 1,
        WizardStep::PublishDid => 2,
        WizardStep::Done => 3,
    }
}

#[derive(Properties, PartialEq)]
pub struct IdentityBootstrapWizardProps {
    /// The page's own origin, e.g. `https://issuer-admin.ds-labs.org`.
    pub endpoint: String,
    #[prop_or_default]
    pub bearer_token: Option<String>,
    /// An existing, incompletely-bootstrapped context to resume. `None`
    /// lets step 1 create one.
    #[prop_or_default]
    pub participant_context_id: Option<String>,
    /// Fired once, when the DID has been published and the wizard reaches
    /// `WizardStep::Done` -- with the participant context ID a caller can
    /// now treat this deployment as bootstrapped with.
    #[prop_or_default]
    pub on_bootstrapped: Callback<String>,
}

#[function_component(IdentityBootstrapWizard)]
pub fn identity_bootstrap_wizard(props: &IdentityBootstrapWizardProps) -> Html {
    let current_step = use_state(|| initial_step(props.participant_context_id.as_deref()));
    let participant_id = use_state(|| props.participant_context_id.clone());

    let on_created = {
        let current_step = current_step.clone();
        let participant_id = participant_id.clone();
        Callback::from(move |created_id: String| {
            participant_id.set(Some(created_id));
            current_step.set(WizardStep::Activate);
        })
    };

    let on_activated = {
        let current_step = current_step.clone();
        Callback::from(move |()| current_step.set(WizardStep::PublishDid))
    };

    let on_published = {
        let current_step = current_step.clone();
        let participant_id = participant_id.clone();
        let on_bootstrapped = props.on_bootstrapped.clone();
        Callback::from(move |()| {
            current_step.set(WizardStep::Done);
            if let Some(id) = &*participant_id {
                on_bootstrapped.emit(id.clone());
            }
        })
    };

    let stepper = html! {
        <ProgressStepper>
            <ProgressStepperStep
                status={step_status(WizardStep::CreateContext, *current_step)}
                is_current={*current_step == WizardStep::CreateContext}
                description="Establish the identity and its first signing key"
            >
                { "Create context" }
            </ProgressStepperStep>
            <ProgressStepperStep
                status={step_status(WizardStep::Activate, *current_step)}
                is_current={*current_step == WizardStep::Activate}
                description="Turn the identity on"
            >
                { "Activate" }
            </ProgressStepperStep>
            <ProgressStepperStep
                status={step_status(WizardStep::PublishDid, *current_step)}
                is_current={*current_step == WizardStep::PublishDid}
                description="Make the DID publicly resolvable"
            >
                { "Publish DID" }
            </ProgressStepperStep>
        </ProgressStepper>
    };

    let body = match *current_step {
        WizardStep::CreateContext => html! {
            <ParticipantContextPanel
                endpoint={props.endpoint.clone()}
                bearer_token={props.bearer_token.clone()}
                participant_context_id={None::<String>}
                on_created={on_created}
            />
        },
        WizardStep::Activate => {
            let Some(id) = (*participant_id).clone() else {
                // Unreachable via `initial_step`/`on_created` (both only
                // ever set this step alongside a known ID), but a plain
                // guard is cheaper than an `unwrap` panicking on a future
                // wiring mistake.
                return html!(
                    <Alert r#type={AlertType::Danger} title="No participant context to activate" inline=true />
                );
            };
            html! {
                <ParticipantContextPanel
                    endpoint={props.endpoint.clone()}
                    bearer_token={props.bearer_token.clone()}
                    participant_context_id={Some(id)}
                    on_activated={on_activated}
                />
            }
        }
        WizardStep::PublishDid => {
            let Some(id) = (*participant_id).clone() else {
                return html!(
                    <Alert r#type={AlertType::Danger} title="No participant context to publish a DID for" inline=true />
                );
            };
            html! {
                <DidPublishPanel
                    endpoint={props.endpoint.clone()}
                    bearer_token={props.bearer_token.clone()}
                    participant_context_id={id.clone()}
                    participant_id={id}
                    on_published={on_published}
                />
            }
        }
        WizardStep::Done => {
            let id = (*participant_id).clone().unwrap_or_default();
            html! {
                <>
                    <Alert r#type={AlertType::Success} title="Identity bootstrapped" inline=true>
                        <p>{ format!("\"{id}\" is active and its DID is published.") }</p>
                    </Alert>
                    <br />
                    <Title level={Level::H3} size={Size::Large}>{ "Keys" }</Title>
                    <KeypairLifecyclePanel
                        endpoint={props.endpoint.clone()}
                        bearer_token={props.bearer_token.clone()}
                        participant_context_id={id}
                    />
                </>
            }
        }
    };

    html! {
        <>
            { stepper }
            <br />
            { body }
        </>
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn starts_at_create_context_with_no_existing_id() {
        assert_eq!(initial_step(None), WizardStep::CreateContext);
    }

    #[test]
    fn skips_straight_to_activate_when_an_id_is_already_known() {
        assert_eq!(initial_step(Some("did.ds-labs.org")), WizardStep::Activate);
    }

    #[test]
    fn step_status_marks_earlier_steps_success_current_info_and_later_pending() {
        // `ProgressStepperStepStatus` (patternfly-yew) derives `PartialEq`
        // but not `Debug`, so `matches!` rather than `assert_eq!` here.
        let current = WizardStep::Activate;
        assert!(matches!(
            step_status(WizardStep::CreateContext, current),
            ProgressStepperStepStatus::Success
        ));
        assert!(matches!(
            step_status(WizardStep::Activate, current),
            ProgressStepperStepStatus::Info
        ));
        assert!(matches!(
            step_status(WizardStep::PublishDid, current),
            ProgressStepperStepStatus::Pending
        ));
    }
}

/// Component-level tests of the wizard's step-advancement wiring: which
/// step the stepper marks current (patternfly-yew's real `pf-m-current`
/// class, from the real `ProgressStepperStep` source -- not guessed) for
/// each `participant_context_id` prop shape. Neither test waits on the
/// panels' own network effects to resolve (there is no real identity-api
/// behind this test harness) -- `initial_step`'s synchronous choice is
/// already committed on the very first render, before any `spawn_local`
/// future gets a chance to run, same reasoning as the sibling
/// `dom_tests`/`tests` modules in `ds-authority-governance-ui`.
#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use gloo_timers::future::TimeoutFuture;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    async fn settle() {
        TimeoutFuture::new(0).await;
    }

    fn mount_in_fresh_div(props: IdentityBootstrapWizardProps) -> web_sys::Element {
        let document = web_sys::window().expect("window").document().expect("document");
        let root = document.create_element("div").expect("create_element");
        document.body().expect("body").append_child(&root).expect("append_child");
        yew::Renderer::<IdentityBootstrapWizard>::with_root_and_props(root.clone(), props).render();
        root
    }

    /// The current step's title text only (not its description) -- the
    /// real `ProgressStepperStep` (patternfly-yew) renders both inside the
    /// same `.pf-m-current <li>`, as two separate sibling spans (title,
    /// then description); `.pf-v6-c-progress-stepper__step-title` selects
    /// just the former.
    fn current_step_label(root: &web_sys::Element) -> String {
        root.query_selector(".pf-m-current .pf-v6-c-progress-stepper__step-title")
            .expect("query_selector")
            .expect("a step marked pf-m-current")
            .text_content()
            .unwrap_or_default()
    }

    #[wasm_bindgen_test]
    async fn opens_on_create_context_when_no_participant_context_id_is_given() {
        let root = mount_in_fresh_div(IdentityBootstrapWizardProps {
            endpoint: "https://example.invalid".to_string(),
            bearer_token: None,
            participant_context_id: None,
            on_bootstrapped: Callback::noop(),
        });
        settle().await;

        assert_eq!(current_step_label(&root), "Create context");
        assert!(
            root.text_content().unwrap_or_default().contains("Participant ID"),
            "expected the create form to be showing"
        );

        root.remove();
    }

    #[wasm_bindgen_test]
    async fn skips_to_activate_when_a_participant_context_id_is_already_known() {
        let root = mount_in_fresh_div(IdentityBootstrapWizardProps {
            endpoint: "https://example.invalid".to_string(),
            bearer_token: None,
            participant_context_id: Some("did.ds-labs.org".to_string()),
            on_bootstrapped: Callback::noop(),
        });
        settle().await;

        assert_eq!(current_step_label(&root), "Activate");

        root.remove();
    }
}
