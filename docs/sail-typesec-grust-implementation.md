# QueryGraph Over a Sail Lakehouse

Date: 2026-06-13

This is the target implementation for QueryGraph as an all-Rust AI navigator
over enterprise data. The current repository already demonstrates the first
semantic bundle shape: Croissant, CDIF, DID, and ODRL in JSON-LD. The next
version should treat that bundle as the portable contract around a Sail
lakehouse, use `github.com/querygraph/grust` as the graph substrate, and use
`github.com/querygraph/typesec` as the typed security and DID fabric.

The short version:

- Sail is the warehouse and execution substrate.
- Grust is the graph representation and graph-store abstraction.
- TypeSec is the RBAC/ODRL/typed capability and DID gateway.
- TypeDID is the verifiable identity envelope for agents, tools, bundles, and
  lineage attestations.
- Open Semantic Interchange is the metric and business semantic model.
- Semantic Croissant describes tables, fields, examples, references, and
  dataset loading semantics.
- CDIF is the FAIR/discovery/access/vocabulary/integration projection over the
  same assets.
- OpenLineage is the event stream for registration, validation, query, and
  derivation.

## Product Shape

QueryGraph is an AI navigator for enterprise data. Its job is not to replace a
warehouse, graph database, identity provider, catalog, BI layer, or lineage
service. Its job is to make an agent ask and answer questions safely:

1. Resolve user intent to OSI metrics, dimensions, relationships, and business
   terms.
2. Map those semantic objects to Sail tables and snapshots.
3. Check RBAC and ODRL policy before planning, indexing, translating, training,
   or deriving.
4. Traverse a Grust graph that connects users, roles, policies, semantic
   models, Croissant record sets, CDIF profiles, tables, columns, snapshots,
   OpenLineage runs, and generated answers.
5. Emit OpenLineage events for every material semantic action.
6. Anchor signed bundle and lineage-event hashes in TypeDID/DID attestations so
   an auditor can prove what was known, allowed, and executed at the time.

The first complete product should be a Rust service and CLI:

```text
querygraphd
  /v1/models
  /v1/models/{id}/bundle
  /v1/search
  /v1/plan
  /v1/answer
  /v1/lineage/events
  /v1/audit/attestations

querygraph
  import-osi
  import-croissant
  import-sail-catalog
  build-bundle
  explain-access
  answer
  emit-lineage
  verify-attestation
```

## Layered Architecture

```text
AI client / tool runner
  |
  | TypeDID envelope: signed and encrypted request
  v
QueryGraph Gateway
  |
  | TypeSec: DID verification, typed capabilities, RBAC, ODRL
  v
Navigator Planner
  |
  | OSI metric resolver + Croissant table/field resolver + CDIF discovery
  v
Grust Semantic Graph
  |
  | nodes: agents, models, metrics, record sets, fields, policies, tables,
  |        columns, snapshots, OpenLineage runs, attestations
  | edges: derives_from, describes, governs, can_read, can_index, used_in,
  |        equivalent_to, cites, observed_by
  v
Sail Lakehouse
  |
  | query execution, table metadata, snapshots
  v
OpenLineage Event Sink
  |
  | canonical event hash
  v
TypeDID / DID Attestation Ledger
```

## Rust Workspace Proposal

QueryGraph should become a Rust workspace that depends on the published
QueryGraph crates and sibling checkouts during development:

```toml
[workspace]
members = [
  "crates/querygraph-core",
  "crates/querygraph-osi",
  "crates/querygraph-croissant",
  "crates/querygraph-cdif",
  "crates/querygraph-lineage",
  "crates/querygraph-sail",
  "crates/querygraph-graph",
  "crates/querygraph-security",
  "crates/querygraph-service",
  "crates/querygraph-cli",
]

[workspace.dependencies]
grust = { package = "grust-graph", version = "0.11", features = ["sail"] }
typesec = { version = "0.11", features = ["agent", "rbac", "odrl", "integrations"] }
typesec-core = "0.11"
typesec-rbac = "0.11"
typesec-odrl = "0.11"
typesec-integrations = "0.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = { package = "serde_norway", version = "0.9" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
axum = "0.8"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
uuid = { version = "1", features = ["v4", "serde"] }
```

Crate responsibilities:

- `querygraph-core`: stable identifiers, content hashes, bundle envelope,
  semantic object IDs, errors, and time.
- `querygraph-osi`: Open Semantic Interchange loader, validator, normalizer,
  and dialect selector.
- `querygraph-croissant`: table and field metadata, RecordSet/FileObject
  projection, Semantic Croissant import/export.
- `querygraph-cdif`: CDIF profile projection and FAIR discovery API.
- `querygraph-lineage`: OpenLineage object model, QueryGraph custom facets,
  canonical JSON hashing, and emitter clients.
- `querygraph-sail`: Sail catalog and table adapters, snapshot metadata,
  schema extraction, and query execution boundary.
- `querygraph-graph`: Grust graph schema, node/edge builders, graph indexes,
  traversal queries, and SailGraphStore/PgGraphStore wiring.
- `querygraph-security`: TypeSec policy integration, TypeDID gateway, bundle
  signing/verification, capability checks, and audit attestations.
- `querygraph-service`: HTTP API.
- `querygraph-cli`: operator and integration CLI.

## Core Identifiers

Every object gets two IDs:

- a stable semantic URI for cross-system references;
- a content hash for audit and reproducibility.

Recommended ID forms:

```text
qg:model:{namespace}:{name}
qg:metric:{model_id}:{metric_name}
qg:dataset:{model_id}:{dataset_name}
qg:recordset:{dataset_id}:{recordset_name}
qg:field:{recordset_id}:{field_name}
qg:sail-table:{catalog}:{schema}:{table}
qg:sail-snapshot:{table_id}:{snapshot_id}
qg:policy:{target_id}:{policy_version}
qg:lineage-event:{sha256}
qg:attestation:{issuer_typedid}:{sha256}
```

TypeDID should identify the principal or service asserting the object:

```text
did:typedid:<method-specific-id>#querygraph-navigator
did:typedid:<method-specific-id>#sail-catalog
did:typedid:<method-specific-id>#policy-authority
did:typedid:<method-specific-id>#lineage-emitter
```

The bundle DID should be issuer-controlled, not derived only from local names.
The current deterministic `did:oyd` generator is still useful for local demos,
but production QueryGraph should use TypeSec's `DidResolver`,
`Ed25519DidKeyStore`, and `TypeDidGateway`.

## Semantic Model Ingest

### OSI

Open Semantic Interchange is the business semantic layer. QueryGraph should
load OSI YAML/JSON into typed Rust structs:

```rust
pub struct OsiSemanticModel {
    pub name: String,
    pub description: Option<String>,
    pub ai_context: Option<AiContext>,
    pub datasets: Vec<OsiDataset>,
    pub relationships: Vec<OsiRelationship>,
    pub metrics: Vec<OsiMetric>,
    pub custom_extensions: Vec<Extension>,
}
```

Required ingest behavior:

1. Validate shape and required fields.
2. Normalize names into stable QueryGraph IDs.
3. Preserve the original OSI document and canonical hash.
4. Resolve each `dataset.source` to a Sail table or an external catalog URI.
5. Select metric expressions by dialect at query time, not at registration
   time.
6. Store `ai_context` as graph-searchable text connected to the metric,
   dataset, field, and model nodes.

OSI should not carry identity, usage control, lineage truth, or FAIR discovery
metadata directly. QueryGraph composes those around it.

### Semantic Croissant

Croissant describes the table-facing and ML-facing dataset structure. In this
architecture, every Sail table referenced by OSI gets a Croissant `FileObject`
or table-like source descriptor, and every OSI dataset gets at least one
Croissant `RecordSet`.

Mapping:

```text
OSI semantic_model.name        -> Croissant Dataset.name
OSI dataset.source             -> FileObject.contentUrl / Sail table URI
Sail schema column             -> Croissant Field.source.extract.column
OSI dataset.primary_key        -> Croissant RecordSet.key
OSI relationship               -> Croissant Field.references
OSI field label/description    -> Croissant Field.description
OSI field semantic type        -> Croissant Field.dataType / equivalentProperty
OSI ai_context examples        -> Croissant RecordSet.examples
```

Where OSI is metric/business meaning, Croissant is record/field/data-loading
meaning. A table may have Croissant without OSI. An OSI model that references
tables should always be projected into Croissant for agent-safe table access.

### CDIF

CDIF lives above Croissant and OSI as the cross-domain FAIR projection. It is
not the source of table schema and not the metric DSL.

In QueryGraph, CDIF should be materialized as:

- a JSON-LD `dcat:Dataset` view for each bundle;
- graph nodes for the five profile concerns:
  `discovery`, `data-access`, `controlled-vocabularies`, `data-integration`,
  and `universals`;
- API indexes for search and external exchange;
- the bridge from enterprise lakehouse terms to domain-neutral FAIR metadata.

This answers "where does CDIF live?" CDIF lives at the publication and
interoperability boundary:

```text
Sail table/schema -> Croissant record/field description
OSI model         -> metric and business semantic description
QueryGraph bundle -> CDIF FAIR/discovery/access projection
Grust graph       -> CDIF nodes and edges for search/federation
```

CDIF should refer to controlled vocabularies, ontologies, landing pages,
access services, contact points, units, temporal coverage, spatial coverage,
and standards conformance. It should not duplicate every OpenLineage event or
every row-level policy.

## Graph Model in Grust

Use Grust as the canonical graph representation. The current Grust surface
already provides `Graph`, `Node`, `Edge`, YAML import/export, `GraphStore`,
`GraphAdminStore`, and feature-gated `SailGraphStore`.

Initial node labels:

```text
Agent
TypeDid
Role
Capability
OdrlPolicy
OsiModel
OsiMetric
OsiDataset
CroissantDataset
CroissantRecordSet
CroissantField
CdifProfile
OntologyTerm
SailCatalog
SailTable
SailColumn
SailSnapshot
LineageJob
LineageRun
LineageDataset
LineageEvent
Attestation
Answer
```

Initial edge labels:

```text
asserted_by
signed_by
governed_by
has_capability
permits
prohibits
describes
projects_to
resolved_to
has_field
references
equivalent_to
derived_from
used_input
produced_output
observed_by
anchored_by
cited_by_answer
```

Minimal graph-builder sketch:

```rust
pub fn build_semantic_graph(bundle: &NavigatorBundle) -> grust::prelude::Result<Graph> {
    let mut graph = GraphBuilder::new();

    let model = graph.node(bundle.osi.model_id()).label("OsiModel").finish()?;
    let issuer = graph.node(bundle.issuer.did()).label("TypeDid").finish()?;

    graph.edge(issuer.id(), model.id())
        .label("asserted_by")
        .prop("bundle_hash", bundle.content_hash())
        .finish()?;

    for metric in &bundle.osi.metrics {
        let metric_node = graph.node(metric.id()).label("OsiMetric").finish()?;
        graph.edge(model.id(), metric_node.id()).label("has_metric").finish()?;
    }

    Ok(graph.build())
}
```

For storage, use `SailGraphStore` when graph objects should live inside the
same Sail lakehouse boundary. Use `PgGraphStore` or another Grust backend for
local development, integration tests, and deployments that want graph storage
outside Sail.

## Security and Policy

TypeSec should own authorization. QueryGraph should not grow a parallel policy
engine.

Policy sources:

- enterprise RBAC from identity groups and roles;
- TypeSec typed capabilities for tool execution;
- TypeSec graph policy for relationship-aware decisions;
- ODRL policies from QueryGraph bundles;
- emergency deny lists and legal holds.

Decision flow:

```text
incoming request
  -> TypeDidGateway.open_message
  -> verified sender DID and decrypted payload
  -> resolve requested action:
       read | search | index | translate | derive | train | export | answer
  -> TypeSec capability check
  -> RBAC/graph policy check
  -> ODRL action check on target bundle/table/field
  -> optional row/column policy planning for Sail query
  -> ProtectedTool or query execution
```

Sensitive tool surfaces should be wrapped with `ProtectedTool`. The agent never
gets raw access to Sail; it gets typed tools whose inputs and outputs are
checked and logged.

The executable Dataverse slice in this repo now uses TypeSec in two places:
`A2aTypeDidAdapter` wraps and verifies the QueryGraph agent request as
`typedid/a2a`, and the Ollama path uses `DidMessageGateway` plus
`DidOllamaClient::chat_verified_prompt_bound`. That second path mints
`AiCanInfer` and `CanReadSensitive` capabilities before revealing the governed
prompt to Ollama, then returns a signed DID reply envelope for audit.
The same slice also joins a small RBAC role assignment with the generated ODRL
policy before permitting the answer action.

## OpenLineage

OpenLineage is the operational history. QueryGraph should emit events for:

- OSI model registration and replacement;
- Croissant import or regeneration;
- CDIF bundle publication;
- policy change;
- graph rebuild;
- query planning;
- query execution over Sail;
- answer generation;
- export, indexing, translation, derivation, and model-training attempts.

Use standard facets wherever possible:

- schema facets for Sail input/output tables;
- column lineage facets for query outputs;
- ownership facets for model and table stewards;
- datasource/version facets for Sail table snapshots.

Add QueryGraph custom facets under a `queryGraph_*` prefix with immutable
schema URLs:

```json
{
  "queryGraph_semanticModel": {
    "_producer": "https://querygraph.ai/querygraphd/0.1.0",
    "_schemaURL": "https://querygraph.ai/schemas/openlineage/querygraph-semantic-model-facet/0.1.0.json",
    "osiModelId": "qg:model:finance:revenue",
    "osiModelHash": "sha256:...",
    "navigatorBundleDid": "did:typedid:...",
    "navigatorBundleHash": "sha256:...",
    "selectedMetrics": ["total_revenue"],
    "dialect": "SAIL_SQL"
  },
  "queryGraph_policyDecision": {
    "_producer": "https://querygraph.ai/querygraphd/0.1.0",
    "_schemaURL": "https://querygraph.ai/schemas/openlineage/querygraph-policy-decision-facet/0.1.0.json",
    "decision": "allow",
    "principalDid": "did:typedid:...",
    "capabilities": ["CanReadRevenueMetric"],
    "odrlPolicyHash": "sha256:..."
  }
}
```

## DID Ledger and Lineage History

Do not put full OpenLineage history in a DID ledger.

OpenLineage events can be numerous, private, and operationally noisy. A DID
ledger should anchor proofs, not become the lineage warehouse. The right design
is:

1. Store full OpenLineage events in the lineage backend.
2. Canonicalize each event as JSON.
3. Hash each event.
4. Batch event hashes into a Merkle tree by model, table, tenant, and time
   window.
5. Sign the Merkle root with the TypeDID issuer responsible for the emitter.
6. Store the attestation in the DID-linked audit ledger.
7. Store a pointer from the attestation to the OpenLineage event store.

Attestation shape:

```json
{
  "@type": "querygraph:LineageAttestation",
  "issuer": "did:key:z...",
  "subject": "qg:model:finance:revenue",
  "timeRange": {
    "from": "2026-06-13T00:00:00Z",
    "to": "2026-06-13T01:00:00Z"
  },
  "openLineageNamespace": "sail://enterprise/finance",
  "eventCount": 182,
  "merkleRoot": "sha256:...",
  "eventStore": "s3://audit-bucket/openlineage/2026/06/13/00/",
  "signature": {
    "type": "Ed25519Signature2020",
    "verificationMethod": "did:key:z...#key-1",
    "value": "..."
  }
}
```

The current qg-rust implementation uses TypeSec `Ed25519DidKeyStore` to derive
the lineage issuer DID, sign the event root, verify it locally, and optionally
append the attestation to a JSONL DID-ledger sink with `--did-ledger-file`.
Full OpenLineage events can be written with `--openlineage-file` or POSTed to a
lineage endpoint with `--openlineage-url`; the ledger receives only the signed
root attestation.

This gives auditors ledger-grade non-repudiation without turning DIDs into a
data lake. It also lets QueryGraph answer: "Which signed semantic bundle and
which signed lineage window governed this answer?"

## Query Planning

The agent answer path:

```text
question
  -> embed/search over Grust graph
  -> candidate OSI metrics, datasets, fields, ontology terms
  -> TypeSec preflight:
       can the principal discover these objects?
  -> resolve candidate metrics to Sail tables and dialect expressions
  -> TypeSec enforcement:
       can the principal read/index/derive/export each target?
  -> build Sail query plan
  -> execute through querygraph-sail
  -> emit OpenLineage START/RUNNING/COMPLETE or FAIL
  -> build answer with cited graph/table/lineage evidence
  -> sign answer metadata or return inside TypeDID reply envelope
```

The answer object should never be just text:

```rust
pub struct NavigatorAnswer {
    pub answer_text: String,
    pub cited_nodes: Vec<GraphNodeId>,
    pub cited_tables: Vec<SailTableRef>,
    pub cited_snapshots: Vec<SailSnapshotRef>,
    pub policy_decisions: Vec<PolicyDecisionReceipt>,
    pub lineage_events: Vec<OpenLineageEventId>,
    pub bundle_hashes: Vec<Hash>,
    pub attestation_refs: Vec<AttestationRef>,
}
```

## API Surface

HTTP endpoints:

```text
POST /v1/models/import/osi
POST /v1/models/import/croissant
GET  /v1/models
GET  /v1/models/{model_id}
GET  /v1/models/{model_id}/bundle
GET  /v1/models/{model_id}/cdif
GET  /v1/models/{model_id}/metrics
POST /v1/search
POST /v1/plan
POST /v1/answer
POST /v1/memory/remember
POST /v1/memory/recall
POST /v1/memory/forget
GET  /v1/lineage/events/{event_id}
GET  /v1/audit/attestations/{attestation_id}
POST /v1/audit/verify
```

Every endpoint that exposes non-public content should accept a TypeDID
envelope or a transport-level authenticated request that is converted into a
local verified TypeDID view.

The implemented memory routes are stricter than the general compatibility
mode: they always require an Ed25519 TypeDID envelope, bind its sender to the
verification-method `did:key`, and use that verified DID as the TypeSec RBAC
subject. `querygraph-memory` persists the vault in Turso/libSQL; see
[`memory-service.md`](memory-service.md) for the operational contract.

## Implementation Phases

### Phase 1: Rust Bundle Core

- Move the current `qg-rust` bundle code into workspace crates.
- Add OSI typed loader and canonical hash.
- Add Croissant import/export with table-shaped `RecordSet` and `Field`
  support.
- Add CDIF projection from OSI + Croissant + Sail metadata.
- Keep output compatible with the existing `querygraph:AiNavigatorSemanticBundle`.

### Phase 2: TypeSec Integration

- Replace local-only DID generation with TypeSec DID resolution/signing.
- Add `TypeDidGateway` for signed/encrypted agent requests.
- Add TypeSec policy checks for `read`, `search`, `index`, `derive`, `train`,
  `export`, and `answer`.
- Wrap Sail and graph tools in `ProtectedTool`.

### Phase 3: Grust Graph

- Define GraphSchema for QueryGraph semantic nodes and edges.
- Project every bundle into a Grust graph.
- Store graph in `SailGraphStore` for Sail-local deployments and another
  Grust backend for dev/test.
- Add traversal queries for "find relevant data I am allowed to use."

### Phase 4: Sail Lakehouse Adapter

- Discover Sail catalogs/tables/schemas/snapshots.
- Resolve OSI `dataset.source` to Sail tables.
- Generate Croissant field metadata from Sail schemas.
- Execute planned queries through a typed Sail boundary.

### Phase 5: OpenLineage and Audit

- Emit OpenLineage for import, planning, execution, and answer generation.
- Add QueryGraph custom facets.
- Hash events and build TypeDID-signed Merkle attestations.
- Add `/v1/audit/verify`.

The current vertical slice emits an answer-generation `COMPLETE` event with
QueryGraph custom facets, can append it to a local JSONL lineage log or POST it
to an OpenLineage endpoint, hashes the event, and stores only a TypeSec
Ed25519-signed root attestation in the DID-ledger JSONL sink.

### Phase 6: Ontologies and Federation

- Import OWL/RDF/SKOS or JSON-LD ontology terms into Grust.
- Map OSI fields and Croissant fields to ontology IRIs.
- Publish CDIF views for cross-domain discovery.
- Support inbound signed bundles from partner QueryGraph nodes.

## What This Makes Possible

With this architecture, QueryGraph can answer enterprise questions with a
real evidence chain:

```text
The answer used OSI metric total_revenue from bundle sha256:...
That metric resolved to Sail table finance.orders at snapshot 91823.
The requester had CanReadRevenueMetric and odrl:read permission.
The query emitted OpenLineage run 6b7... with schema and column-lineage facets.
The lineage window was anchored by TypeDID attestation qg:attestation:...
The response cites Croissant fields orders.amount and orders.order_date and
CDIF discovery profile finance-revenue-dataset.
```

That is the platform: an AI navigator that can find relevant enterprise data,
explain why it is relevant, prove who asserted its semantics, enforce RBAC and
ODRL, run over Sail, and leave an auditable OpenLineage and TypeDID trail.

## References

- Open Semantic Interchange: https://open-semantic-interchange.org/
- MLCommons Croissant: https://mlcommons.org/working-groups/data/croissant/
- Croissant specification: https://docs.mlcommons.org/croissant/docs/croissant-spec.html
- CODATA CDIF: https://codata.org/initiatives/making-data-work/cdif/
- OpenLineage facets: https://openlineage.io/docs/spec/facets/
- OpenLineage specification: https://github.com/OpenLineage/OpenLineage/blob/main/spec/OpenLineage.md
- TypeSec: https://github.com/querygraph/typesec
- Grust: https://github.com/querygraph/grust
