# Announcing the QueryGraph stack: Lobster, Torcello, Ocelot, Sentinel

![The QueryGraph fleet: a governed graph of ships, radios, and lighthouse beacons.](../../cover/querygraph-blog-headboard.png)

QueryGraph is an AI Navigator over governed enterprise data. The thesis is not "throw the warehouse at a model." Serious questions are local, contextual, permissioned, and reproducible. An agent should be able to answer a hard question over enterprise data while knowing what the data is, where it came from, who is asking, which action is allowed, and how to replay the run.

The current stack now has a coordinated substrate wave:

- **Grust 0.12.0 "Lobster"** — the backend-neutral graph and GQL/Cypher substrate.
- **TypeSec 0.12.0 "Torcello"** — typed authority and wire-level tool-call governance.
- **LakeCat 0.3.0 "Ocelot"** — governed Iceberg REST, OpenLineage evidence, and QGLake handoff proof.
- **QueryGraph 0.4.0 "Sentinel"** — Rust/Python semantic import, verification, and navigator APIs over those substrates.

These are not independent libraries that happen to share an organization. The seams are intentional: LakeCat consumes Grust and TypeSec, emits QGLake artifacts, and QueryGraph verifies/imports those artifacts. TypeSec tracks Grust for graph-shaped policy. QueryGraph Rust and Python test against each other so the semantic layer is not a one-language story.

The finished companion book, [*The QueryGraph Stack*](https://firstpair.org/read/querygraph/), is now in the First Pair library: read it as a fixed-layout PDF or reflowable EPUB from its library card, open the complete single-page web edition at that link, or move through it in the [chapter-by-chapter reader](https://firstpair.org/read/querygraph/chapters/). It was built using our innovative First Pair method: Alexy and AI worked together as an authoring pair—Alexy setting the thesis, architecture, evidence bar, and editorial direction while AI investigated the code and standards, drafted and revised the text, built every format, and verified the released artifacts.

## Grust 0.12 "Lobster": the graph substrate gets precise

Grust is a backend-neutral property-graph library for Rust: labeled nodes and edges, typed properties, graph builders, traversal IR, schema metadata, mutation contracts, backend adapters, and a GQL/Cypher language layer over the same model.

Lobster is the release where Grust's GQL/Cypher claim becomes precise enough to be useful. The supported profile is enumerated in a machine-readable manifest and pinned by tests. CALL subqueries, table-valued functions, shortest paths, graph values, session/catalog surfaces, metadata DDL, native-query escape hatches, and atomic transaction batches all land inside an honest profile statement. The remaining strict-write rejections are documented as correctness boundaries, not hand-waved gaps.

This matters for QueryGraph because the semantic layer is a graph, not a pile of JSON. Datasets, fields, policies, agents, lineage events, catalog objects, and handoff proofs need a queryable substrate that can run locally, in Turso, in PostgreSQL, through Sail/Spark paths, or behind other graph backends.

The Grust Lobster post is the first piece of this announcement wave.

## TypeSec 0.12 "Torcello": authority crosses the wire

TypeSec turns authority into typed evidence. A privileged function can require a `Capability<P, R>`; that capability has no public constructor; it is minted only by policy. Torcello carries that model into agent tool calls.

The new interop plane guards OpenAI, Anthropic, LangChain, Pydantic AI, and MCP tool-call shapes with the same deny-by-default `ToolCallGuard`. It adds MCP and OpenAI/Anthropic proxy gateways, policy-aware tool listing, JSON-Schema argument guards, signed decision receipts, OpenTelemetry spans, decision logs and replay, a `#[typesec_tool]` macro, Python adapters, and WASM/JS bindings.

For QueryGraph, this is the difference between an agent that merely says it followed policy and an agent whose tool calls, receipts, and replay logs are policy-shaped from the start.

## LakeCat 0.3 "Ocelot": catalog state becomes proof

LakeCat is a Rust-native Iceberg REST catalog foundation. It keeps the standard Iceberg boundary thin: identity, tenancy, metadata-pointer state, policy gates, and integration events live in LakeCat; Sail owns Iceberg planning, Grust owns graph behavior, TypeSec owns governance, and QueryGraph owns semantic import and navigation.

Ocelot is release-candidate proven from a clean tree. Its full local gate verifies standard contracts, Rust feature matrices, book artifacts, Grust/TypeSec/Sail integration rows, all-features tests, and the QGLake handoff. That handoff starts LakeCat locally, plans through Sail, writes Turso catalog state, projects a Grust Turso catalog graph, drains OpenLineage evidence, and runs QueryGraph's locked verify/import commands over the same bundle.

The artifacts are concrete: `lakecat-bootstrap.json`, `lineage-drain.json`, `querygraph-import-plan.json`, and `handoff-summary.json`. The summary is schema-closed and hash-bound. Extra proof claims are rejected. This is the catalog boundary becoming something a downstream navigator can verify.

## QueryGraph 0.4 "Sentinel": the navigator layer

QueryGraph sits above those substrates. The Rust implementation provides the reference semantic projections, governance story, LakeCat loaders, lineage and TypeDID evidence, CLI, and API. The Python implementation mirrors the same layer with Pydantic v2 models, LangChain/MCP hooks, validation, and a cross-language equivalence suite.

The current contract is not aspirational. `qg-rust` is at 0.4.0 and depends on Grust 0.12, LakeCat 0.3, and TypeSec 0.12. `qg-python` is also 0.4.0. The Python tests run the Rust CLIs and assert that the important semantic outputs agree.

The purpose is a governed answer loop:

1. discover relevant semantic graph context,
2. verify catalog and lineage evidence,
3. enforce TypeSec policy at tool and data boundaries,
4. emit TypeDID/OpenLineage evidence,
5. replay the run rather than trusting the prompt transcript.

## Why this matters outside QueryGraph

Each community gets a different doorway:

- **Apache Ossie:** QueryGraph can be a concrete implementation testbed for vendor-neutral semantic graph artifacts.
- **Apache Iceberg:** LakeCat proves that table/catalog workflows can carry portable policy, credential, scan, and lineage evidence.
- **Apache Polaris:** Polaris can remain the Iceberg REST catalog while QueryGraph/LakeCat explore semantic exports and proof adjuncts.
- **OpenLineage:** LakeCat turns governed catalog activity into replayable, hash-bound lineage evidence.
- **Apache Spark / Sail:** governed scan context can travel with Spark-compatible planning instead of living only in application code.
- **Rust:** Grust, TypeSec, and LakeCat are serious Rust infrastructure for graph, catalog, policy, and agent systems.
- **LangChain and Pydantic:** TypeSec gives agent frameworks typed tool governance, receipts, and memory/data provenance hooks.

QueryGraph is still early, but the shape is now visible: graph substrate, typed authority, governed catalog, and semantic navigator, all release-aligned and tested together. The next step is not one giant platform announcement. It is a series of focused conversations with the communities that already own pieces of the problem.
