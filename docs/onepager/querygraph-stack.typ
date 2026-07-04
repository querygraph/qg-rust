// The QueryGraph Stack — one-pager (typst edition).
// Build: typst compile querygraph-stack.typ querygraph-stack-typst.pdf

#set page(paper: "us-letter", margin: (x: 0.75in, y: 0.65in))
#set text(font: "Helvetica", size: 9.6pt)
#set par(justify: false)

#let accent = rgb("#1a5fb4")
#let dim = rgb("#5e5c64")
#let section(title) = {
  v(7pt)
  text(size: 10.5pt, weight: "bold", fill: accent, upper(title))
  v(3pt)
}

#text(size: 21pt, weight: "bold")[The QueryGraph Stack]
#v(-4pt)
#text(size: 11pt, fill: dim)[A governed semantic lakehouse for agentic AI — five
coordinated open-source components that let agents answer over enterprise data
while _proving_ what they did.]

#section[The thesis]
#block(fill: rgb("#f7f9fc"), stroke: (left: 2pt + accent), inset: 7pt, radius: 2pt)[
  “The answer used OSI metric *X*, resolved to Sail table *Y*, under capability
  *C* and `odrl:read`, emitting OpenLineage run *R* anchored by attestation
  *A* — and source *Z* was denied, with a receipt.” The differentiator for
  enterprise AI is not a longer context window; it is this verifiable chain
  from question to answer. Denials are receipts, never errors.
]

#section[The components]
#table(
  columns: (auto, 1fr, auto),
  stroke: 0.4pt + rgb("#d0d0d5"),
  inset: 5pt,
  [*Component*], [*Role*], [*Release*],
  [Grust], [Backend-neutral property graph for Rust; full GQL/Cypher reads
    (Full39075: CALL subqueries, TVFs, shortestPath); a dozen stores including
    Sail DataFrames.], [0.12.0 “Lobster”],
  [TypeSec], [Security in the type system: unforgeable `Capability<P, R>`
    proofs; TypeDID Ed25519 signed agent envelopes; framework guards, MCP
    gate, enforcement proxy.], [0.12.0 “Torcello”],
  [LakeCat], [Rust-native Iceberg REST catalog with stock-client conformance;
    catalog state, Sail planning, receipts, and graph projection bound to one
    table transition; QueryGraph bootstrap bundles.], [0.3.0 “Ocelot”],
  [Sail (fork)], [Spark-compatible lakehouse engine with a Cypher graph-query
    extension compiled into the SQL frontend.], [branch `grust`],
  [QueryGraph], [The semantic layer, Rust + Python: Croissant · CDIF · DID ·
    ODRL projections, OSI business semantics, RBAC+ODRL dual gate,
    OpenLineage + attestations.], [0.3.0 “Goshawk”],
)

#section[Goshawk: the interoperability release]
*Real Ed25519 across languages* — Python signs, Rust verifies; Rust signs,
Python verifies; same seed ⇒ same `did:key`; tampering fails on either side.
*HTTP `/v1` API* with TypeDID envelope auth (path- and body-bound; 401s carry
receipts). *MCP servers in both languages* — Claude, LangChain, PydanticAI,
LlamaIndex, CrewAI connect with zero adapters. *A2A agent card* published
identically by both implementations. *Official OpenLineage conformance* —
events schema-validated, deterministic UUIDv5 run ids shared across languages.
The 0.4 line adds the *governed navigator loop*: question → semantic search →
policy receipts → SQL plans over allowed sources only → any LLM (or the
deterministic baseline) → signed envelope + lineage + attestation.

#section[Held equivalent by tests]
An executable cross-language contract: byte-identical navigator bundles,
matching governance semantics in the multi-agent story, both crypto directions
verified live, both CLIs' lineage events schema-valid, identical agent cards,
and a live auth round-trip — 49 Python + 40 Rust tests, CI on every push.

#v(7pt)
#line(length: 100%, stroke: 0.4pt + rgb("#d0d0d5"))
#text(size: 8.6pt, fill: dim)[
  *Links:* github.com/querygraph/#{"{querygraph, qg-rust, qg-python, grust, typesec, lakecat}"}
  · lakehq/sail · books: `qg-rust/docs/book` & `docs/guide` · querygraph.ai
]
