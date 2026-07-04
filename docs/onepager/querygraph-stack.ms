.\" The QueryGraph Stack — one-pager (troff/ms edition).
.\" Build: groff -Tpdf -P-e -t -ms querygraph-stack.ms > querygraph-stack-troff.pdf
.\"
.\" Formatting choices follow the omnighost troff findings:
.\"   -P-e  embeds fonts (unembedded base-14 fonts cause word-gap artifacts
.\"         when viewers substitute different metrics);
.\"   -t    runs the tbl preprocessor (the component table below);
.\"   .na   sets ragged-right — this text is dense with unhyphenatable
.\"         tokens (did:key, /v1, odrl:read) that stretch justified lines.
.na
.hy 1
.nr PS 10
.nr VS 12
.nr HM 0.65i
.nr FM 0.65i
.nr LL 7.0i
.nr PO 0.75i
.ds CH
.LP
.ps 20
.vs 22
.B "The QueryGraph Stack"
.ps 10
.vs 12
.sp 0.3v
.I "A governed semantic lakehouse for agentic AI"
\(em five coordinated open-source components that let agents answer over
enterprise data while
.I proving
what they did.
.sp 0.5v
.SH
THE THESIS
.LP
\(lqThe answer used OSI metric \fBX\fP, resolved to Sail table \fBY\fP,
under capability \fBC\fP and \fCodrl:read\fP, emitting OpenLineage run
\fBR\fP anchored by attestation \fBA\fP \(em and source \fBZ\fP was denied,
with a receipt.\(rq
The differentiator for enterprise AI is not a longer context window; it is
this verifiable chain from question to answer. Denials are receipts, never
errors.
.SH
THE COMPONENTS
.LP
.TS
allbox tab(|);
lb lb lb
l lw(3.4i) l.
Component|Role|Release
Grust|T{
Backend-neutral property graph for Rust; full GQL/Cypher reads (Full39075:
CALL subqueries, TVFs, shortestPath); a dozen stores including Sail
DataFrames.
T}|0.12.0 \(lqLobster\(rq
TypeSec|T{
Security in the type system: unforgeable Capability<P, R> proofs; TypeDID
Ed25519 signed agent envelopes; framework guards, MCP gate, enforcement proxy.
T}|0.12.0 \(lqTorcello\(rq
LakeCat|T{
Rust-native Iceberg REST catalog with stock-client conformance; catalog
state, Sail planning, receipts, and graph projection bound to one table
transition; QueryGraph bootstrap bundles.
T}|0.3.0 \(lqOcelot\(rq
Sail (fork)|T{
Spark-compatible lakehouse engine with a Cypher graph-query extension
compiled into the SQL frontend.
T}|branch grust
QueryGraph|T{
The semantic layer, Rust + Python: Croissant, CDIF, DID, ODRL projections,
OSI business semantics, RBAC+ODRL dual gate, OpenLineage + attestations.
T}|0.3.0 \(lqGoshawk\(rq
.TE
.SH
GOSHAWK: THE INTEROPERABILITY RELEASE
.LP
.B "Real Ed25519 across languages"
\(em Python signs, Rust verifies; Rust signs, Python verifies; same seed
means the same
.CW did:key ;
tampering fails on either side.
.B "HTTP /v1 API"
with TypeDID envelope auth (path- and body-bound; 401s carry receipts).
.B "MCP servers in both languages"
\(em Claude, LangChain, PydanticAI, LlamaIndex, CrewAI connect with zero
adapters.
.B "A2A agent card"
published identically by both implementations.
.B "Official OpenLineage conformance"
\(em events schema-validated, deterministic UUIDv5 run ids shared across
languages. The 0.4 line adds the
.B "governed navigator loop" :
question \(-> semantic search \(-> policy receipts \(-> SQL plans over
allowed sources only \(-> any LLM (or the deterministic baseline) \(->
signed envelope + lineage + attestation.
.SH
HELD EQUIVALENT BY TESTS
.LP
An executable cross-language contract: byte-identical navigator bundles,
matching governance semantics in the multi-agent story, both crypto
directions verified live, both CLIs' lineage events schema-valid, identical
agent cards, and a live auth round-trip \(em 49 Python + 40 Rust tests,
CI on every push.
.sp 0.4v
.ds HR \l'\n(.lu\(ul'
\*(HR
.br
.ps 8
Links: github.com/querygraph/{querygraph, qg-rust, qg-python, grust, typesec,
lakecat} \(bu lakehq/sail \(bu books: qg-rust/docs/book & docs/guide \(bu
querygraph.ai
.ps 10
