# Changelog

All notable changes to the QueryGraph Rust reference implementation are
recorded here. The codename pool and the shared version line live in
[`RELEASES.md`](RELEASES.md).

## 0.5.0-dev — unreleased

### Added
- **Worked examples throughout the stack guide** (`docs/guide`): each
  component chapter now ends with runnable Rust and/or Python examples with
  outputs captured from real runs — graph build/store/Cypher, capability
  minting, one-seed-two-languages envelope signing, the LakeCat bootstrap
  handoff, Croissant→OSI in both languages, the navigator loop with its
  denial receipt, sign-in-Python/verify-in-Rust, the guarded `/v1` 401
  contract and `governed_post`, a hand-driven MCP session, and tool-schema
  export in both flavors.

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
