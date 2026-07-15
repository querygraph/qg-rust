# Changelog

All notable changes to the QueryGraph Rust reference implementation are
recorded here. The codename pool and the shared version line live in
[`RELEASES.md`](RELEASES.md).

## 0.5.0-dev — unreleased

### Added
- **Persistent capability-secured agent memory**: `querygraph-memory` now
  connects qg-rust to a bootstrapped, file-backed Turso/libSQL graph through
  TypeSec's `MemoryVault`. `serve --memory-policy … --memory-db …` enables
  signed-only `/v1/memory/{remember,recall,forget}` routes. The verified
  TypeDID `did:key`—not a body field—is the policy subject; the signature key
  must match that sender, and the envelope recipient must be the QueryGraph
  service DID. Calls pass through `ToolCallGuard`, typed capability minting,
  clearance-aware recall, and the persistent vault. Router tests prove
  unsigned rejection, cross-recipient replay rejection, body-subject spoof
  resistance, RBAC denial, and close/reopen persistence.
- **The stack guide restructured as a full book** (`docs/guide`): executive
  summary and overview up front; four Parts — I. The Substrate (Grust, the
  query language, TypeSec, TypeDID, LakeCat, the bootstrap handoff, Sail),
  II. The Semantic Layer (a chapter per standard: Croissant, CDIF, DID,
  ODRL, plus OSI, the dual gate, lineage, the lakehouse path, the QGLake
  story, qg-python), III. The Interoperability Surfaces (`/v1` + envelope
  auth, MCP, A2A + tool schemas, the navigator loop, the cross-language
  contract), IV. Integration in Practice (the eleven-step assembly, catalog to
  governed answer, plugging in agent frameworks, operating and releasing) —
  closed by Future Work and a glossary/link appendix. 27 chapters; worked
  Rust/Python examples throughout, with outputs (bundle layers, receipts,
  verification reports, MCP transcripts) captured from real runs.
- **Per-chapter API references in the stack guide**: compact reference tables
  for every surface — Grust builder/stores/Cypher, TypeSec capabilities and
  TypeDID, LakeCat REST + the bundle crate, the four projection types in both
  languages, OSI, governance, lineage, the qg-python package map, `/v1` auth,
  the MCP tools, the navigator loop, and both CLIs.
- **A second integration walkthrough over live Dataverse data** (guide
  Chapter 25): `dataverse-e2e` against Harvard Dataverse, with output from a
  real run — live search staged into Sail, derived semantics, the dual-gate
  receipt, the `typedid/a2a` envelope, and DOI-level OpenLineage with a
  UUIDv5 run id and Ed25519 attestation.
- **Dual typesettings for the stack guide**: `-typst` and `-troff` PDF/EPUB
  editions alongside the canonical build. The troff PDF is set with
  `groff -Tpdf -P-e -k -t -ms` (embedded fonts, preconv, tbl) over a
  regenerated gropdf font map, with a `pdffonts` embed assertion; code-fence
  language tags are stripped for the ms writer (pandoc's highlight token
  macros are standalone-only and render blank otherwise). Both books' iCloud
  publishing now prunes superseded versioned copies before delivering.

### Changed
- **TypeSec Lido alignment**: the qg-rust agent, policy, TypeDID, and Marciana
  dependencies now resolve the `0.13.0` TypeSec release, keeping fresh local
  builds and the persistent memory integration on the same substrate line.
- **Marciana guide refresh**: the stack book now treats TypeSec 0.13.0
  "Lido" as the current security substrate, explains its capability-secured
  memory contract and QueryGraph `/v1` boundary, walks the Pydantic AI v2
  restart proof, and separates shipped v1 guarantees from post-v1 scale and
  hosted-service work. The verified suite ledger is now 41 Rust and 52 Python
  tests.

## 0.4.0 "Sentinel" — 2026-07-04

The governed-answer release: where Goshawk opened the doors (MCP, A2A, `/v1`,
cross-language crypto), Sentinel stands guard over what comes through them —
envelope auth on the API, the governed navigator loop with receipts, Rust
minting the envelopes Python verifies, and the whole stack realigned to the
0.12 substrate wave (Grust "Lobster", TypeSec "Torcello", LakeCat "Ocelot").
Ships alongside qg-python 0.4.0 "Sentinel".

### Changed
- **Stack alignment to the 0.12 substrate wave**: Grust `0.11.0 "Crab"` →
  `0.12.0 "Lobster"` (merged Full39075 GQL profile, atomic Cypher transaction
  batches), TypeSec `0.11.0 "Burano"` → `0.12.0 "Torcello"` (the
  agent-interoperability platform release), LakeCat `0.2.1 "Lynx"` → `0.3.0
  "Ocelot"` (stock-client Iceberg REST conformance). All 40 tests green
  against the new line; both books, the stack guide, the deck, the one-pager,
  and the READMEs updated accordingly.

### Added
- **The QueryGraph Stack guide** (`docs/guide`) — a second book: the
  definitive stack-wide guide (Grust, TypeSec, LakeCat, Sail, QueryGraph)
  with an executive summary and link index up front, built to EPUB/PDF/MOBI
  with versioned delivery links like the dedicated book, which remains in
  `docs/book` and gains a Goshawk interoperability chapter.
- **Stack review deck** (`docs/slides`, typst → PDF) and a **one-pager**
  (`docs/onepager`) in three typesettings: HTML, typst PDF, and troff/ms PDF.
  The troff build applies the omnighost findings — `groff -Tpdf -P-e -t -ms`
  (embedded fonts, `tbl` preprocessing, ragged-right) — regenerates the
  gropdf font map against the installed ghostscript, and asserts every font
  embeds via `pdffonts`.
- **MCP server over stdio** (`mcp` module; CLI: `mcp-serve`). A
  dependency-free JSON-RPC 2.0 implementation of the MCP handshake
  (protocol 2024-11-05) exposing the same governed surface as `/v1` and
  qg-python's FastMCP server: `build_navigator_bundle`, `run_qglake_story`,
  `verify_envelope`, `import_semantic_model` (OSI or Croissant),
  `search_semantic_models`, and `answer_question` (shared deterministic
  answer core with `/v1/answer`). Pointable at Claude Code/Desktop and any
  MCP client.
- **TypeDID envelope auth on `/v1`** (`serve --require-auth`). Governed routes
  (`models/import/*`, `answer`) demand a signed envelope in `x-qg-envelope`:
  `action == "invoke"`, `resource` bound to the request path (no cross-route
  replay), `payload.bodySha256` bound to the body, Ed25519 signature checked
  against the envelope's did:key. Failures are 401s carrying a receipt and
  the auth contract. Open routes (health, GETs, agent card, verify) stay open.
- **Rust now mints qg-python-compatible envelopes**
  (`PyTypeDidEnvelope::signed`): identical seed → did:key derivation as
  Python's `Ed25519Signer.from_seed`, closing the reverse crypto direction
  (Rust signs → Python verifies).
- **`POST /v1/answer`, first slice**: semantic search over the model
  registry, SQL plans for the matches, deterministic synthesis, and a signed
  TypeDID envelope plus an OpenLineage run with a spec-conformant UUID. The
  fully governed loop (RBAC+ODRL receipts, pluggable LLMs) is qg-python's
  `GovernedNavigatorLoop`; Rust parity follows with envelope auth.

## 0.3.0 "Goshawk" — 2026-07-03

The interoperability release, implementing FABLE-REVIEW-1 alongside qg-python
0.3.0 "Goshawk" (see the workspace `FABLE-REVIEW-1.md` §9).

### Added
- **A2A Agent Card** (`a2a` module; served at `/.well-known/agent-card.json`;
  CLI: `agent-card`). Aligns the existing `typedid/a2a` protocol label with
  the Linux Foundation Agent2Agent protocol: skills mirror the `/v1` surface
  and the security scheme documents the TypeDID envelope contract. The skill
  list is a cross-language contract asserted against qg-python by the
  equivalence suite.
- **`/v1` semantic-model registry**: `POST /v1/models/import/{osi,croissant}`
  (Croissant JSON-LD projects to OSI via the new
  `OsiDocument::from_croissant_json`, mirroring qg-python), `GET /v1/models`,
  `GET /v1/models/{name}`, and `GET /v1/search?q=` over names, descriptions,
  ai_context, semantic types, and ontology terms.
- **Cross-language envelope verification** (`agent::interop`). Rust now
  verifies qg-python's Ed25519-signed TypeDID envelopes with no shared state:
  `did:key` resolution (multibase/multicodec), reconstruction of the documented
  `querygraph-typedid-signing-v1` signing payload, `ed25519-dalek`
  verification, and byte-exact recomputation of Python's canonical payload JSON
  (`json.dumps(..., sort_keys=True, separators=(",", ":"))` with
  `ensure_ascii` escaping). Golden fixture generated by qg-python is tested in
  `cargo test`; the live round-trip (Python signs → Rust verifies, tampering
  rejected) runs in qg-python's equivalence suite.
- **`verify-envelope` CLI command**: reads an envelope JSON from a file or
  stdin, prints the verification report, exits non-zero unless the signature
  verifies.
- **`/v1` HTTP API, first slice** (`server` module, axum; CLI: `serve --port`).
  The platform is reachable over a network for the first time
  (FABLE-REVIEW-1 §4.1): `GET /v1/health`, `POST /v1/navigator/bundle`
  (four-layer Croissant/CDIF/DID/ODRL bundle), `GET /v1/qglake/story` (the
  governed multi-agent evidence chain), and `POST /v1/audit/verify-envelope`
  (verifies qg-python Ed25519 envelopes; an invalid signature is a 200 with a
  receipt, not a server error). Router-level tests cover all endpoints,
  including tamper rejection.
- **GitHub Actions CI**: fmt, clippy `-D warnings`, and tests against
  checkouts of `querygraph/grust` and `querygraph/lakecat` assembled to
  satisfy the `../..` path dependencies.

### Changed
- **OpenLineage run ids are now spec-conformant UUIDs**: the official 2-0-2
  JSON Schema requires `run.runId` to be a UUID, so run ids are deterministic
  UUIDv5 values under the QueryGraph namespace
  (`uuid5(NAMESPACE_URL, "https://querygraph.ai/openlineage")`), derived from
  the same seeds as before (envelope signatures, bundle hashes). qg-python
  derives identical ids; both CLIs' emitted events now validate against the
  official schema in the equivalence suite.

## 0.2.0 "Peregrine" — 2026-06-26

See the release log in [`RELEASES.md`](RELEASES.md).
