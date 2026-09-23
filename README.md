# ds-identity-bootstrap-ui

Reusable [Yew](https://yew.rs) components for the DCP identity-bootstrap
concern shared by two otherwise-different audiences: a connector operator
(participant) setting up their own IdentityHub tenant, and an authority
operator setting up the tenant IdentityHub uses to represent the authority
itself. Both need the same three actions against EDC IdentityHub's
`identity-api` (`ParticipantContextPanel`, `DidPublishPanel`,
`KeypairLifecyclePanel`) -- there's nothing authority-specific or
participant-specific about creating a participant context, publishing its
`did:web` document, or rotating its signing key.

Deliberately a component **library**, not a standalone app: no `Trunk.toml`,
no `index.html`. Consumers (e.g.
[`ds-authority-governance-ui`](https://github.com/ds-labs-org/ds-authority-governance-ui))
mount these components inside their own router/layout.

Built on [`edc-identity-hub-client`](https://github.com/ds-labs-org/edc-identity-hub-client)
(the `ds-labs-org` fork, `feat/v1beta-and-issuer-admin-api` branch) for the
actual `identity-api` calls, and [`patternfly-yew`](https://github.com/patternfly-yew/patternfly-yew)
for styling, at the same versions as `ds-authority-governance-ui` and
`dataspace-rs/edc-web-ui` so components look at home embedded in either.

## Components

- `ParticipantContextPanel` -- create / view / activate / deactivate /
  delete a participant context.
- `DidPublishPanel` -- publish / unpublish / query the `did:web` document
  for a participant context.
- `KeypairLifecyclePanel` -- rotate / revoke / activate a participant
  context's signing key.

Each takes the participant context ID and an `IdentityHubClient` (or its
endpoint/auth config) as props -- the embedding app owns routing, auth, and
which participant context is "current."

## Testing

`wasm-bindgen-test` for component-level tests (headless browser), run with:

```bash
wasm-pack test --headless --chrome
```

Pure logic (props validation, request/response mapping) has no
`yew`/`wasm-bindgen` dependency and is covered by plain `cargo test`.
