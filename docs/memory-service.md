# QueryGraph Persistent Memory Service

Status: implemented and locally verified on 2026-07-14.

This is the operational contract for the qg-rust Marciana integration. The
service assembles three existing boundaries without weakening any of them:

1. qg-python signs each HTTP request with a TypeDID Ed25519 credential;
2. qg-rust binds the verified `did:key` sender to a TypeSec policy decision;
3. `querygraph-memory` persists the capability-gated vault in Turso/libSQL.

## Start the service

Memory is disabled unless a policy is supplied. The database directory is
created automatically and Grust bootstraps its universal graph tables on open.

```bash
cargo run -- serve \
  --port 8080 \
  --memory-policy memory-policy.yaml \
  --memory-db .querygraph/memory.db
```

`--require-auth` still controls the older model-import and answer routes.
Memory routes are always authenticated because a verified identity is required
to mint a memory capability.

## Policy

Policy subjects must be the public `did:key` identities carried in signed
envelopes, not agent display names or private seeds:

```yaml
roles:
  - name: shared-research-memory
    permissions: [read, write, delete]
    resources: ["memory/team:marciana/shared"]
assignments:
  - subject: "did:key:z6MkSpecialist..."
    roles: [shared-research-memory]
  - subject: "did:key:z6MkSupervisor..."
    roles: [shared-research-memory]
```

Keep signing seeds in the client dependency container or secret manager. Only
public DIDs belong in policy files.

## HTTP contract

Every request needs `Content-Type: application/json` and `x-qg-envelope`.
The envelope must have:

- `action: "invoke"`;
- `recipient: "did:web:qg-server"`;
- `resource` equal to the exact request path;
- `payload.bodySha256` equal to the request body's SHA-256;
- a valid Ed25519 signature whose verification-method DID equals `sender`.

The bodies are:

```json
{"space":"memory/team:marciana/shared","text":"a governed finding","kind":"semantic","purpose":"research"}
```

```json
{"space":"memory/team:marciana/shared","query":"governed","clearance":"internal","purpose":"research"}
```

```json
{"space":"memory/team:marciana/shared","ids":["mem-..."]}
```

Successful responses identify the authenticated subject and wrap the standard
TypeSec memory result:

```json
{"allowed":true,"subject":"did:key:z6Mk...","result":{"hits":[],"redacted":[]}}
```

Authentication failures are `401`, policy denials are `403` with a receipt,
and a correctly signed request against a server started without memory is
`503`. A JSON `subject` field is ignored; identity comes only from the verified
envelope.

## Verification and restart behavior

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The router suite proves unsigned rejection, signature/sender binding, body
subject spoof resistance, exact-DID RBAC denial, and close/reopen persistence.
The Grust suite separately runs the full TypeSec store conformance corpus,
transactional consolidation, and nested-Tokio tests against the same Turso
adapter.

The end-to-end client demonstration is:

```bash
cd ../qg-python
uv sync --extra crypto --extra pydantic-ai
uv run python examples/pydantic_ai_v2_memory_agents.py
```

It uses Pydantic AI v2 `Capability` objects for credentials and memory, restarts
qg-rust between write and read, and shows an unassigned signed DID being denied.

## V1 boundary

This delivery includes durable local persistence, policy isolation, and an
executable application/client path. Native LanceDB ANN, Sail-distributed
consolidation, fuller GQL temporal/lineage pushdown, persistent anti-replay
state, quotas, migrations, and hosted multi-tenant operations remain post-v1.
