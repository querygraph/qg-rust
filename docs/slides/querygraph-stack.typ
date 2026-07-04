// The QueryGraph Stack — review deck.
// Build: typst compile querygraph-stack.typ  (or ./build.sh)

#set page(width: 13.33in, height: 7.5in, margin: (x: 0.9in, y: 0.7in), fill: white)
#set text(font: "Helvetica", size: 21pt)

#let accent = rgb("#1a5fb4")
#let dim = rgb("#5e5c64")

#let slide(title, body) = page[
  #text(size: 30pt, weight: "bold", fill: accent)[#title]
  #v(0.5em)
  #body
]

// ── Title ────────────────────────────────────────────────────────────────
#page[
  #v(22%)
  #align(center)[
    #text(size: 46pt, weight: "bold")[The QueryGraph Stack]
    #v(0.3em)
    #text(size: 24pt, fill: dim)[A governed semantic lakehouse for agentic AI]
    #v(2em)
    #text(size: 18pt)[Grust · TypeSec · LakeCat · Sail · QueryGraph]
    #v(0.6em)
    #text(size: 15pt, fill: dim)[Stack review · QueryGraph 0.4.0 “Sentinel” · querygraph.ai]
  ]
]

// ── Thesis ───────────────────────────────────────────────────────────────
#slide[The thesis: prove what the agent did][
  - Enterprise AI's differentiator is not a bigger context window — it is a
    *verifiable chain* from question to answer.
  - Every answer carries: the semantics used, the policies that allowed it,
    the sources touched, *and the sources denied* — with receipts.
  - #text(fill: accent)[“The answer used OSI metric X, resolved to Sail table Y,
    under capability C and odrl:read, emitting OpenLineage run R anchored by
    attestation A — and source Z was denied, with a receipt.”]
  - No mainstream agent framework offers that sentence.
]

// ── Stack at a glance ────────────────────────────────────────────────────
#slide[Five components, one evidence chain][
  #table(
    columns: (auto, 1fr, auto),
    stroke: 0.5pt + dim,
    inset: 9pt,
    [*Component*], [*Role*], [*Release*],
    [Grust], [Backend-neutral property graph + GQL/Cypher reads], [0.12.0 “Lobster”],
    [TypeSec], [Capabilities as types; TypeDID signed envelopes], [0.12.0 “Torcello”],
    [LakeCat], [Rust Iceberg REST catalog; QueryGraph bootstrap bundles], [0.3.0 “Ocelot”],
    [Sail (fork)], [Spark-compatible engine + Cypher extension], [branch `grust`],
    [QueryGraph], [The governed semantic layer, Rust + Python], [0.4.0 “Sentinel”],
  )
]

// ── Grust ────────────────────────────────────────────────────────────────
#slide[Grust: the graph substrate][
  - `Graph = nodes + edges`; ids, labels, typed properties — backend-neutral.
  - One facade crate, many stores: memory, SurrealDB, FalkorDB, HelixDB,
    LanceDB, PostgreSQL (incl. SQL/PGQ), Turso, *Sail DataFrames*.
  - `grust-cypher`: Cypher/GQL reads over any store.
  - QueryGraph's semantic graph — datasets, fields, ontology terms, agents,
    policies — is a Grust graph.
  - New in Lobster (0.12): the merged Full39075 GQL profile — CALL
    subqueries, TVFs, shortestPath, passthrough — + atomic batch transactions.
]

// ── TypeSec ──────────────────────────────────────────────────────────────
#slide[TypeSec: security in the type system][
  - `Capability<CanWrite, Report>` is an *unforgeable proof* — crate-private
    constructor, phantom types, sealed permissions.
  - No capability ⇒ the guarded function *does not exist* for your code.
    Violations are compile errors.
  - TypeDID: Ed25519 keys from seeds, did:key documents, signed + encrypted
    agent envelopes with audit-safe attestations.
  - New in Torcello (0.12): framework interop plane (OpenAI/Anthropic/
    LangChain/Pydantic-AI guards), deny-by-default `mcp-gate`, enforcement
    proxy, signed decision receipts + replay.
]

// ── LakeCat ──────────────────────────────────────────────────────────────
#slide[LakeCat: the catalog boundary][
  - Rust-native Apache Iceberg REST catalog (`/catalog/v1`) — standard
    clients just work.
  - Catalog state, Sail planning, TypeSec receipts, and Grust projection bind
    to the *same accepted table transition*.
  - `/querygraph/v1/bootstrap`: live tables projected into Croissant, CDIF,
    OSI, ODRL, OpenLineage, and a Grust-ready envelope — one shared wire
    crate, no copied formats.
  - New in Ocelot (0.3.0): stock-client Iceberg REST conformance (PyIceberg
    round-trip, spec-correct errors), fail-closed v4 validation, recorded
    release proof over the 0.12 substrate.
]

// ── Sail ─────────────────────────────────────────────────────────────────
#slide[Sail + the Cypher extension][
  - Spark-compatible lakehouse engine: typed Parquet tables, Spark Connect,
    PySpark — the compute substrate.
  - The fork compiles a *Cypher graph-query surface into Sail itself*
    (\~5,600 lines: parser AST, analyzer, plan resolver).
  - Reuses Grust's property-graph model — the same semantic graph is
    `MATCH`-able from any Spark session.
  - QueryGraph's audit trail (`qg_audit`) lives next to the data it audits.
]

// ── QueryGraph core ──────────────────────────────────────────────────────
#slide[QueryGraph: four projections + business semantics][
  - Every dataset described four ways: *Semantic Croissant* (ML metadata),
    *CDIF* (discovery/access), *W3C DID* (deterministic identity),
    *ODRL* (rights).
  - *OSI* models map business terms → governed Sail columns, with
    per-dialect SQL and ontology terms.
  - Governance is a dual gate: RBAC *and* ODRL must both allow.
    Denials are receipts, not errors.
  - Lineage: OpenLineage events (official-schema valid, UUIDv5 run ids) +
    Ed25519-anchored attestations.
]

// ── Goshawk interop ──────────────────────────────────────────────────────
#slide[Goshawk (0.3.0): the interoperability release][
  - *Real Ed25519 across languages*: Python signs → Rust verifies, Rust
    signs → Python verifies; same seed ⇒ same did:key. Tampering fails.
  - *HTTP `/v1` API*: bundles, story, model registry + search, answer,
    envelope audit — with TypeDID envelope auth (path- and body-bound).
  - *MCP servers in both languages* — Claude, LangChain, PydanticAI,
    LlamaIndex, CrewAI reach everything with zero adapters.
  - *A2A agent card* at `/.well-known/agent-card.json` — identical skills
    from both implementations, asserted by tests.
]

// ── Navigator loop ───────────────────────────────────────────────────────
#slide[Sentinel (0.4.0): the governed navigator loop][
  #text(size: 19pt)[
    question → semantic search (synonyms, bigrams) → *RBAC+ODRL receipts* →
    SQL plans over allowed sources → synthesis → *signed envelope +
    OpenLineage + attestation*
  ]
  - Denied sources are named in the prompt as off-limits — never planned.
  - LLM is any `Callable[[str], str]`: Ollama, vLLM, llama.cpp, LM Studio via
    one OpenAI-compatible helper; `llm=None` is the deterministic baseline.
  - Same governance with or without a model in the loop.
]

// ── Cross-language contract ──────────────────────────────────────────────
#slide[Held equivalent by tests, not discipline][
  - Navigator bundles: byte-identical modulo timestamps.
  - QGLake story: same roster, only the restricted broker denied,
    field-identical attestation schemas.
  - Crypto: both directions verified live; fixtures pin shared did:key and
    UUIDv5 derivations.
  - OpenLineage events from *both* CLIs validate against the official schema.
  - Live auth: Python-minted header → 200; without → 401 + receipt.
  - 49 Python + 40 Rust tests; CI on every push.
]

// ── Operate ──────────────────────────────────────────────────────────────
#slide[Operating the stack][
  ```
  cargo run -- serve --require-auth      # /v1 + agent card
  cargo run -- mcp-serve                 # MCP over stdio (Rust)
  uv run querygraph mcp-serve --osi m.yaml
  uv run querygraph answer --question "…"
  ```
  - Sibling checkouts: `~/src/{querygraph,grust,lakecat}`; TypeSec from
    crates.io.
  - Coordinated codenamed releases; CHANGELOGs; versioned book artifacts;
    GitHub Actions CI in every repo.
]

// ── Roadmap ──────────────────────────────────────────────────────────────
#slide[Roadmap][
  - Navigator loop: live LLM runs under identical receipts; Rust parity.
  - Remaining `/v1`: lineage queries, audit verification, access explanation.
  - Adopt TypeSec Torcello's surfaces: interop plane, `mcp-gate`,
    enforcement proxy (deps already on 0.12).
  - Polaris `SemanticModel` + `/navigator-bundle` (LakeCat first);
    `OSIMetricFacet` → OpenLineage; dbt/Cube importers; ADBC path.
  - The benchmark: *how much does a governed semantic layer improve agent
    accuracy over the same lakehouse?*
]

// ── Links ────────────────────────────────────────────────────────────────
#slide[Links][
  #set text(size: 19pt)
  - Meta-repo: `github.com/querygraph/querygraph`
  - Implementations: `querygraph/qg-rust` · `querygraph/qg-python`
    (releases v0.4.0 “Sentinel”)
  - Substrate: `querygraph/grust` · `querygraph/typesec` ·
    `querygraph/lakecat` · `lakehq/sail`
  - Books: the dedicated QueryGraph book (`qg-rust/docs/book`) and
    *The QueryGraph Stack* guide (`qg-rust/docs/guide`)
  - Standards: Croissant · CDIF · DID · ODRL · OSI · OpenLineage · MCP · A2A
  #v(1em)
  #align(center)[#text(fill: dim)[querygraph.ai]]
]
