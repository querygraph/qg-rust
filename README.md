# QueryGraph

QueryGraph is a Rust implementation sketch for the AI Navigator semantic layer. It turns dataset-facing metadata into a governed JSON-LD bundle with four explicit layers:

1. Semantic Croissant: ML-ready dataset metadata, file objects, record sets, and fields.
2. CDIF: cross-domain FAIR discovery, access, vocabulary, integration, and universal metadata.
3. DID: decentralized identity for agent and service attribution.
4. ODRL: machine-actionable rights, permissions, prohibitions, and constraints.

The design is informed by QueryGraph.ai’s graph/search direction, AgStack Pale Fire’s knowledge-graph search architecture, CODATA CDIF profiles, Croissant Toolkit skills, CODATA local-agent patterns, The Minority Report’s multilingual controlled-vocabulary workflow, and ODRL/DID attribution patterns for local AI services.

The QueryGraph resources page adds the standards spine:

- W3C DID Core for DID documents, controllers, services, and resolution.
- W3C ODRL Information Model 2.2 for permissions, prohibitions, duties, constraints, and policy profiles.
- CODATA ODRL demo for URL-to-DID anchoring at `/api/did/create_from_url`.
- DDI ISO/PAS 25955:2026 for research/statistical metadata interoperability.
- MLCommons Croissant 1.1 for agent-ready provenance, vocabulary interoperability, and governance.

## Architecture

```text
src/
  agent.rs       TypeDID-shaped agent request, policy receipt, and Ollama-shaped reply
  croissant.rs   Dataset, FileObject, RecordSet, Field -> Croissant JSON-LD
  cdif.rs        CDIF profile projection over a Croissant dataset
  dataverse.rs   Dataverse Native/Search API ingest and Croissant projection
  did.rs         Deterministic local did:oyd identity document
  lakecat.rs     LakeCat QueryGraph bootstrap bundle verification
  lineage.rs     OpenLineage event plus TypeDID-style lineage attestation
  odrl.rs        Policy, permissions, prohibitions, and access checks
  codata.rs      Client for CODATA ODRL demo DID anchoring APIs
  navigator.rs   AI Navigator composition pipeline
  osi.rs         Open Semantic Interchange loader and Dataverse OSI projection
  rbac.rs        Role assignments and resource/action access decisions
  sail.rs        Local Sail staging files and Spark/Sail SQL load plan
  validation.rs  Local Croissant, CDIF, and OpenLineage shape validation
  main.rs        CLI entry point
```

The first product boundary is intentionally narrow: build a semantic bundle that can be indexed by a knowledge graph, served by an agent, or attached to a dataset catalog. Query/search, graph storage, and external policy resolution should build on this library instead of being embedded directly in it.

## Sail, TypeSec, Grust, OSI, and OpenLineage Target

The comprehensive lakehouse implementation proposal is in
[`docs/sail-typesec-grust-implementation.md`](docs/sail-typesec-grust-implementation.md).
It updates the older four-layer sketch into an all-Rust QueryGraph platform:
Sail as the warehouse, Grust as the graph substrate, TypeSec/TypeDID as the
security fabric, Open Semantic Interchange as the business semantic model,
Semantic Croissant as the table/field metadata layer, CDIF as the FAIR
publication projection, and OpenLineage as the audit event stream.

The runnable supervised-agent story is in [`QGLake.md`](QGLake.md). Execute it
with:

```bash
cargo run -- qglake-story
```

It prints a readable Resilience Desk briefing with supervisor, specialist,
restricted broker, and synthesis agents; TypeDID envelopes; RBAC and ODRL
receipts; Semantic Croissant and CDIF projections; and OpenLineage/DID
attestation evidence. Use `cargo run -- qglake-story --json` for the full
machine-readable report.

## Stack versions

This is **QueryGraph 0.4.0 "Sentinel"** (release codenames are birds of prey;
see [`RELEASES.md`](RELEASES.md)). It builds on
three coordinated, named open-source releases. The full story is in the stack
announcement at
[`docs/blog/announcing-querygraph-stack.md`](docs/blog/announcing-querygraph-stack.md)
and the stack guide at [`docs/guide`](docs/guide).

- **Grust 0.12.0 "Lobster"** — the backend-neutral property-graph substrate
  with a standards-conformant GQL/Cypher layer, now completed by the merged
  Full39075 profile: `CALL { … }` subqueries, table-valued functions,
  `shortestPath()`, passthrough escape hatches, and atomic Cypher transaction
  batches.
- **TypeSec 0.12.0 "Torcello"** — the type-safe security fabric grown into an
  agent-interoperability platform: unforgeable capabilities and audit-safe
  TypeDID attestations, plus framework guards (OpenAI/Anthropic/LangChain/
  Pydantic-AI), a deny-by-default MCP gate, signed decision receipts with
  replay, and an OpenAI/Anthropic-compatible enforcement proxy.
- **LakeCat 0.3.0 "Ocelot"** — the thin Iceberg REST catalog boundary with
  governance, lineage, and proof on a Turso MVCC spine, now with stock-client
  Iceberg REST conformance proven by a PyIceberg round-trip. QueryGraph
  verifies its bootstrap bundles through LakeCat's own shared `qglake-bundle`
  crate rather than a hand-maintained copy of the wire format.

## Run

```bash
cargo run -- navigator \
  --dataset-name "Hazard vocabulary" \
  --description "Controlled vocabulary with multilingual technical terms" \
  --landing-page "https://querygraph.ai/datasets/hazards" \
  --data-url "https://querygraph.ai/datasets/hazards.csv"
```

Reproduce the CODATA ODRL demo anchoring behavior for QueryGraph resources:

```bash
cargo run -- anchor-url --url "https://querygraph.ai/resources/"
```

This calls `https://odrl.dev.codata.org/api/did/create_from_url` and returns the `did:oyd` identifier plus the stored payload reported by the demo service.

Verify a LakeCat QueryGraph bootstrap bundle before importing its semantic
artifacts or loading catalog graph projections:

```bash
cargo run -- lakecat-verify --bundle lakecat-bootstrap.json
```

The verifier recomputes the LakeCat manifest hashes for Croissant, CDIF, OSI,
ODRL, OpenLineage artifacts, and the outer bundle hash, failing before graph
import if anything does not match the bundle manifest.

Write a QueryGraph import plan after verification:

```bash
cargo run -- lakecat-import \
  --bundle lakecat-bootstrap.json \
  --output .querygraph/lakecat/import-plan.json
```

The import plan records verified tables, semantic artifact labels, and the
catalog graph size after validating the LakeCat graph envelope through Grust.
This command is the QueryGraph-side acceptance handoff.

Run the Dataverse-to-Sail-to-agent vertical slice without external services:

```bash
cargo run -- dataverse-e2e \
  --sail-dir .querygraph/sail \
  --question "Which governed datasets mention access control?"
```

That command uses Dataverse-shaped fixture datasets, projects the first dataset
to Croissant/CDIF/DID/ODRL, stages all datasets as JSONL files for a local Sail
session, emits Spark/Sail `CREATE OR REPLACE TEMP VIEW ... USING json` load
statements, checks the generated ODRL policy, wraps the agent request with
TypeSec's TypeDID envelope code, and returns an Ollama-shaped answer report.
The access receipt requires both a demo RBAC role assignment and the generated
ODRL policy before the agent is allowed to answer.

To use a live Dataverse installation:

```bash
cargo run -- dataverse-e2e \
  --dataverse-url "https://demo.dataverse.org" \
  --query "climate" \
  --limit 3 \
  --sail-dir .querygraph/sail
```

For restricted Dataverse content, pass `--api-token "$DATAVERSE_API_TOKEN"`.
To supply a hand-authored Open Semantic Interchange model instead of the
Dataverse-derived one, pass `--osi-path path/to/model.osi.yaml`. To also anchor
the first dataset landing page through the CODATA ODRL demo API, add
`--anchor-codata`.

To send the generated governed prompt to a local Ollama-compatible server
through TypeSec:

```bash
cargo run -- dataverse-e2e \
  --sail-dir .querygraph/sail \
  --call-ollama \
  --ollama-url "http://localhost:11434" \
  --ollama-model "llama3.2"
```

This path creates a DID-encrypted prompt envelope, verifies it through
TypeSec's `DidMessageGateway`, mints `AiCanInfer` and `CanReadSensitive`
capabilities, calls `DidOllamaClient::chat_verified_prompt_bound`, and records
the signed reply envelope in `agentRun.ollama_typedid`.

The staged JSONL files can be loaded into a running local Sail server using the
`bootstrap_sql` values printed in the report. Start Sail separately, for
example:

```bash
sail spark server --port 50051
```

Then run the live Grust/Sail path:

```bash
cargo run -- dataverse-e2e \
  --sail-dir .querygraph/sail \
  --live-sail \
  --sail-endpoint "http://127.0.0.1:50051" \
  --anchor-codata \
  --openlineage-file .querygraph/openlineage/events.jsonl \
  --openlineage-sail-schema qg_audit \
  --did-ledger-file .querygraph/did-ledger/attestations.jsonl \
  --call-ollama \
  --ollama-model "llama3.2" \
  --question "Which governed datasets mention access control? Answer in one sentence."
```

The report's `sail.graph` object is the live evidence: `loaded_nodes`,
`loaded_edges`, `verified_node_id`, and `verified_node_label` come from
`grust::SailGraphStore` loading and reading back the QueryGraph semantic graph
through the Sail server. The `views` entries in the same object prove that the
Dataverse metadata/files were staged into Sail as Arrow IPC temp views and
queried back with row counts. The top-level `osi` object is the OSI semantic
model used for the graph projection; by default it is synthesized from
Dataverse subjects and keywords as ontology terms. The report's
`agentRun.request.protocol` should be `typedid/a2a`, showing the request was
wrapped through TypeSec's TypeDID adapter. When `--call-ollama` is present,
`agentRun.ollama_typedid` proves the answer came from a TypeSec-verified DID
prompt and a signed DID reply envelope. `agentRun.access.rbac.allowed` and
`agentRun.access.odrl_allowed` should both be true. The `openLineage` object carries a
`COMPLETE` event, a canonical event hash, optional emission receipts, and a
TypeSec Ed25519-signed root attestation showing how the lineage event is
anchored without storing full operational history in the DID ledger. Use
`--openlineage-file` for a local JSONL event sink, `--openlineage-url` to POST
the event to an OpenLineage-compatible endpoint, and `--did-ledger-file` to
append the signed root attestation to a DID-ledger-style JSONL file. Live Sail
runs also append the OpenLineage event and the TypeDID attestation into Sail
audit tables under `qg_audit` by default; use `--openlineage-sail` to force the
same audit sink without `--live-sail`.

## Test

```bash
cargo test
```

## Lakehouse Corpus

The `lakehouse/` subproject downloads the diverse Dataverse/CODATA corpus,
generates Croissant and CDIF sidecars per dataset, streams parseable CSV/TSV/XLSX
files into typed Sail Parquet tables, and records non-tabular assets in the
lakehouse catalog tables.

```bash
sail spark server --port 50051
cargo run -- lakehouse-load --root .querygraph/lakehouse --schema qg_lakehouse
cargo run -- lakehouse-verify --report .querygraph/lakehouse/manifest/load-report.json
cargo run -- lakehouse-validate --report .querygraph/lakehouse/manifest/load-report.json \
  --openlineage-file .querygraph/openlineage/events.jsonl
```

The live inspection session id is `querygraph-lakehouse-loader`.
Set `DATAVERSE_API_TOKEN` or pass `--api-token` to `lakehouse-load` when
restricted Dataverse files require authentication.

## Next Milestones

- Load the staged JSONL table views directly through Spark Connect instead of
  only loading the semantic graph through `grust::SailGraphStore`.
- Add an official OpenLineage JSON Schema compatibility test against the target
  backend used in deployment.
- Generalize the current single-event signed root to batched Merkle roots by
  tenant/model/time window.
- Add official JSON Schema validation for incoming Dataverse, Croissant, CDIF,
  and OSI metadata.
