# Announcing the QueryGraph stack: Lobster, Lido, Ocelot, Sentinel—and Marciana

![The QueryGraph fleet: a governed graph of ships, radios, and lighthouse beacons.](../../cover/querygraph-blog-headboard.png)

QueryGraph is an AI Navigator over governed enterprise data. Its premise is not “put the warehouse in a prompt.” Serious questions are local, contextual, permissioned, and reproducible. An agent should be able to answer a hard question while knowing what the data means, where it came from, who is asking, which action is allowed, and how another operator can replay the run.

The stack now has four named releases and one important post-release integration:

- **[Grust 0.12.0 “Lobster”](https://github.com/querygraph/grust/blob/main/docs/blog/grust-lobster/post.md)** — the backend-neutral property-graph and GQL/Cypher substrate.
- **[TypeSec 0.13.0 “Lido”](https://github.com/querygraph/typesec/releases/tag/v0.13.0)** — typed authority, TypeDID identity, guarded tools, and the released [`typesec-memory` Marciana subsystem](https://github.com/querygraph/typesec/blob/main/docs/blog/announcing-typesec-memory/post.md).
- **[LakeCat 0.3.0 “Ocelot”](https://querygraph.ai/announcing-lakecat/)** — governed Iceberg REST, Turso catalog state, OpenLineage evidence, and the QGLake handoff proof.
- **[QueryGraph 0.4.0 “Sentinel”](https://github.com/querygraph/qg-rust/releases/tag/v0.4.0)** — the tagged Rust/Python semantic import, verification, and navigator release.
- **Current QueryGraph main** — the unreleased post-Sentinel line, now using TypeSec 0.13 and carrying durable, identity-bound Marciana memory plus a native Pydantic AI v2 proof.

These are not independent libraries that happen to share an organization. They form one evidence path:

```text
Pydantic AI v2 / MCP / agent framework
  -> signed TypeDID request and TypeSec capability check
  -> QueryGraph semantic search and governed answer loop
  -> LakeCat catalog state and Sail planning/execution
  -> Grust graph traversal and Turso/libSQL persistence
  -> Croissant + CDIF + OSI + ODRL + OpenLineage evidence
  -> governed Marciana memory for a later authorized agent
```

The components remain separately usable, but their seams are deliberate. LakeCat consumes Grust and TypeSec and emits a QGLake bundle that QueryGraph verifies. Sail executes the lakehouse work. QueryGraph Rust and Python agree on the semantic and identity contracts. TypeSec governs both tool calls and memory. Grust supplies the durable graph beneath both catalog projections and recall.

## Grust 0.12 “Lobster”: the graph substrate

Grust is the backend-neutral property-graph layer: labeled nodes and edges, typed properties, graph builders, traversal IR, schema metadata, mutations, and a GQL/Cypher language surface over a common model. The same graph-shaped Rust can target memory, Turso, PostgreSQL, Sail/Spark, SurrealDB, FalkorDB, LanceDB, and other adapters without moving application semantics into a backend-specific client.

Lobster makes the language claim precise. Its machine-readable ISO/IEC 39075 profile realizes 69 of the 74 features Grust set out to support; the other five are intentional strict-write rejections that preserve correctness. `CALL` subqueries, table-valued functions, shortest paths, graph values, catalog/session surfaces, metadata DDL, native-query escape hatches, and atomic transaction batches all sit behind tests that keep the code and profile statement synchronized.

The newer connection is **`querygraph-memory`**. It implements the shared TypeSec memory-store contract on Grust and provides qg-rust with durable Turso/libSQL records, temporal state, entity relationships, neighborhood recall, and transactional consolidation. The graph finds candidates; TypeSec’s vault decides whether plaintext may be revealed.

Read the [Lobster release post](https://github.com/querygraph/grust/blob/main/docs/blog/grust-lobster/post.md) and the broader [Grust architecture introduction](https://querygraph.ai/grust-backend-neutral-property-graphs-for-rust/).

## TypeSec 0.13 “Lido”: authority extends into memory

TypeSec turns authority into typed evidence. A privileged Rust function can require a `Capability<P, R>` whose constructor is not public and which policy alone can mint. The Torcello release carried that model across OpenAI, Anthropic, LangChain, Pydantic AI, MCP, Python, and WASM tool-call shapes through the same deny-by-default `ToolCallGuard`, with policy-aware tool listing, receipts, replay, and proxy enforcement.

[TypeSec 0.13 “Lido”](https://github.com/querygraph/typesec/releases/tag/v0.13.0) keeps that tool-governance plane and adds **Marciana**, the released `typesec-memory` subsystem. Marciana applies information-flow rules to durable agent memory:

- records carry a sensitivity label and a receiving context declares its clearance ceiling;
- purpose, retention, validity time, and observation time are policy inputs rather than prompt conventions;
- model- or user-derived records can be quarantined and excluded from ordinary recall;
- consolidation inherits the labels of its sources instead of laundering derived information;
- forgetting is a separate capability-gated tombstone operation; and
- Rust, Python, WASM, MCP, and backend-conformance surfaces share the same authorization model.

There are two important boundaries. The TypeSec core supports purpose-tagged records, expiry and retention, bi-temporal history, quarantine, consolidation, semantic and neighborhood recall, and optional deletion receipts. qg-rust deliberately exposes a narrower HTTP v1: `remember`, `recall`, and `forget`. Its current router writes conversation provenance with an `Internal` default label; request purpose participates in policy and recall context but is not yet stored as a per-record purpose, and qg-rust does not enable TypeSec’s optional receipts feature. That distinction keeps the shipped vertical slice honest while the broader Marciana core remains reusable.

Read the [Marciana technical announcement](https://github.com/querygraph/typesec/blob/main/docs/blog/announcing-typesec-memory/post.md) alongside the [Lido 0.13 release](https://github.com/querygraph/typesec/releases/tag/v0.13.0).

## Sail: the execution layer

[Sail](https://firstpair.press/announcing-sail-rust-book) is the Rust, Spark-compatible lakehouse engine underneath the QueryGraph execution story. QueryGraph uses Sail for Spark Connect, DataFrame and SQL planning, Iceberg-aware work, and governed scans over allowed sources. The QueryGraph fork also carries a Cypher extension so graph queries can execute inside the engine that already owns the data path.

The ownership boundary matters: Sail plans and executes; it does not become the policy system or the semantic catalog. TypeSec decides what is authorized, Grust owns graph behavior, LakeCat owns the Iceberg REST boundary, and QueryGraph composes the governed navigator loop. The [Sail Rust Book announcement](https://firstpair.press/announcing-sail-rust-book) explains that engine and its own codebase-first companion volume.

## LakeCat 0.3 “Ocelot”: catalog state becomes proof

[LakeCat](https://querygraph.ai/announcing-lakecat/) is a thin, Rust-native Iceberg REST catalog foundation. Identity and tenancy, metadata-pointer state, policy gates, transactional outbox events, and integration evidence live at the catalog boundary. Sail owns planning, Grust owns graph projection, TypeSec owns authority, and QueryGraph owns higher-level semantic import and navigation.

Ocelot’s release-candidate gate runs the QGLake handoff end to end. It starts LakeCat, plans through Sail, writes Turso catalog state, projects the catalog into a Grust Turso graph, drains OpenLineage evidence, and runs QueryGraph’s locked verify/import commands over the same bundle. `lakecat-bootstrap.json`, `lineage-drain.json`, `querygraph-import-plan.json`, and `handoff-summary.json` are schema-closed and hash-bound; extra proof claims are rejected rather than ignored.

This is why LakeCat is more than a catalog-shaped service. An accepted table transition, its governed scan context, graph projection, lineage event, and downstream semantic import can all be checked against the same state change. Read the full [LakeCat 0.3 “Ocelot” announcement](https://querygraph.ai/announcing-lakecat/).

## qg-rust and qg-python: one semantic contract in two languages

The tagged [QueryGraph 0.4 “Sentinel” release](https://github.com/querygraph/qg-rust/releases/tag/v0.4.0) established the navigator layer. qg-rust provides reference projections, TypeDID authentication, RBAC and ODRL decisions, LakeCat verification/import, lineage and attestation evidence, CLI workflows, and the HTTP service. qg-python exposes the same semantic shapes to Python applications, agent frameworks, and notebook/lakehouse users, with cross-language tests checking the important outputs rather than asking readers to trust two parallel implementations.

The semantic bundle is standards-shaped:

- **Semantic Croissant** says what files, record sets, and fields a dataset contains.
- **CDIF** says how the resource is discovered, evaluated, and accessed across domains.
- **Open Semantic Interchange (OSI)** supplies business datasets, fields, metrics, relationships, and governed expressions.
- **DID and TypeDID** identify the caller and bind an Ed25519 envelope to the exact recipient, route, action, and body.
- **ODRL plus RBAC** decide what that identity may do; both gates must allow.
- **OpenLineage** records the run and the inputs, outputs, policy evidence, and attestations needed to replay it.

Current qg-rust main is post-Sentinel development, not a new tagged QueryGraph release. It now depends on the released TypeSec 0.13 crates and adds Marciana at the service boundary without changing the status of QueryGraph 0.4. That distinction is useful: the stable release name remains Sentinel while the integration can be inspected and exercised before the next QueryGraph tag.

## The Pydantic AI v2 proof: memory across identity and restart

The new executable story is in qg-python’s [`pydantic_ai_v2_memory_agents.py`](https://github.com/querygraph/qg-python/blob/main/examples/pydantic_ai_v2_memory_agents.py). Each agent receives two separate native Pydantic AI capabilities:

- `querygraph.typedid-credential` keeps its Ed25519 signing seed in typed runtime dependencies; and
- `querygraph.marciana-memory` supplies governed remember, recall, and forget tools backed by qg-rust.

The live request path is:

```text
Pydantic AI v2 agent
  -> TypeDID signed envelope
  -> qg-rust authentication
  -> ToolCallGuard
  -> MemoryToolRouter
  -> MemoryVault
  -> querygraph-memory
  -> Grust on Turso/libSQL
```

The model turn uses Pydantic AI’s deterministic `TestModel`, so no provider key is required, but the credential signatures, HTTP service, TypeSec policy decisions, qg-rust process restart, and Turso database are real. The proof performs five distinct checks:

1. an unsigned memory request returns `401`;
2. an authorized specialist obtains a governed result and remembers it;
3. qg-rust is terminated and restarted against the same Turso file;
4. a different authorized supervisor DID recalls the durable record; and
5. a validly signed outsider reaches authentication but receives a TypeSec `403` policy denial.

The body cannot choose its authority subject, the model never sees private signing material, and restarting the service does not erase governed memory. qg-rust exposes no raw graph endpoint for recall: content returns through the TypeSec vault or not at all.

## One governed answer loop

Taken together, the stack now supports a concrete sequence:

1. discover business meaning in OSI and field-level meaning in Croissant;
2. resolve the governed data product through CDIF and LakeCat;
3. apply RBAC and ODRL to the verified TypeDID caller;
4. plan only allowed tables through Sail;
5. traverse related entities and policies through Grust;
6. produce the answer through the qg-rust/qg-python contract;
7. emit OpenLineage and signed evidence for the run; and
8. remember the result through Marciana only when a separate memory capability permits it.

That is the QueryGraph thesis in operational form: the graph, catalog, compute engine, policy layer, agent framework, and memory system do not merely coexist. They agree on identity, state, semantics, and evidence.

## Read *The QueryGraph Stack*

The finished companion book, [*The QueryGraph Stack: The Definitive Guide to the Governed Semantic Lakehouse*](https://firstpair.org/books/querygraph/), follows the complete architecture from Grust, TypeSec 0.13 and Marciana through LakeCat, Sail, qg-rust, qg-python, Pydantic AI v2, standards, proofs, and the end-to-end navigator loop.

The [First Pair library card](https://firstpair.org/books/querygraph/) is the stable starting point. From there you can download the [fixed-layout PDF](https://firstpair.org/querygraph/pdf/) or [reflowable EPUB](https://firstpair.org/querygraph/epub/), open the [complete web edition](https://firstpair.org/read/querygraph/), or use the [chapter-by-chapter reader](https://firstpair.org/read/querygraph/chapters/).

The book was built with the innovative **First Pair** method: Alexy and AI worked as an authoring pair. Alexy set the thesis, architecture, evidence bar, and editorial direction; AI investigated the code and standards, drafted and revised the manuscript, built each reading format, and verified the released artifacts. The result is not documentation written around an imagined platform. It is a technical book continuously reconciled with the implementation it explains.
