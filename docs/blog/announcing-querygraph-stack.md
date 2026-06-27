# Announcing the QueryGraph Stack: three named releases under one AI Navigator

QueryGraph is an AI Navigator over governed enterprise data. The thesis has
never been "throw the whole warehouse at a model" — it is that serious questions
are local, contextual, permissioned, and reproducible, and they deserve an
architecture built for that reality. An agent should be able to answer a hard
question over enterprise data without turning the enterprise into an unbounded
prompt: it should know what the data is, where it came from, who is asking,
which action is allowed, and how to replay the run.

Today that architecture stands on three coordinated, named open-source releases
that ship from the [QueryGraph org](https://github.com/querygraph) and were cut
to work together:

- **Grust 0.11.0 "Crab"** — the graph and query substrate.
- **TypeSec 0.10.0 "Murano"** — the typed security and governance fabric.
- **LakeCat 0.2.0 "Lynx"** — the Iceberg REST catalog boundary.

They are not three independent libraries that happen to coexist. Murano tracks
Crab; Lynx consumes both as published crates. The versions are coordinated on
purpose, so the seams between graph, policy, and catalog are typed, not hopeful.

## Grust 0.11 "Crab": the graph + query substrate

Grust is a backend-neutral property-graph library for Rust — one graph model of
labeled nodes and edges, typed properties, and stable IDs, over many storage and
execution engines (in-memory, Sail/Spark, PostgreSQL, Turso, and more). Crab is
the first *named* Grust release, and it is a big one: it adds a real,
standards-conformant **GQL/Cypher language layer** built as an honest pipeline —
span-bearing lexer, recursive-descent parser, AST, semantic analysis — on top of
the property graph. The bounded read subset lowers into backend SQL (Spark and
SQLite dialects) with results that are byte-identical to the reference executor
by construction, and a differential SQLite oracle keeps them that way.

Crab also brings first-class **Decimal, Duration, and temporal values** with
lossless arithmetic and chronological ordering, catalog procedures
(`CALL db.labels()`, `db.relationshipTypes()`, `db.propertyKeys()`), a
transaction command surface with honest per-backend atomicity reporting, and
Turso MVCC concurrent writes. The graph stops being only a store of facts and
becomes a queryable substrate. Full post:
[github.com/querygraph/grust/blob/main/docs/blog/grust-crab.md](https://github.com/querygraph/grust/blob/main/docs/blog/grust-crab.md).

## TypeSec 0.10 "Murano": the typed security/governance fabric

Most authorization systems answer *is this allowed?* and then trust every line
of code after the check to remember the answer. TypeSec closes that gap by
turning authority into a value the compiler tracks: a `Capability<P, R>` is
unforgeable proof that permission `P` was granted over resource `R`, it has no
public constructor, and the only way to mint one in production runs a policy
engine and emits an audit event. Forgetting the guard becomes a type error.

Murano carries one policy contract behind many engines — RBAC, ODRL, and a graph
engine that compiles policy into a typed Grust graph with deny-overrides
semantics — plus typestate agents, typed privacy labels, and **DID/TypeDID agent
messaging** with real cryptography. The payoff for QueryGraph is the
**audit-safe attestation**: when agents collaborate over governed data, a
TypeDID envelope records who did what to which resource, at which privacy level,
without ever exposing the payload or the signing material. Murano tracks Grust
0.11 "Crab," and it is API-compatible for consumers across the 0.8→0.10 line.
Full post:
[github.com/querygraph/typesec/blob/main/docs/blog/announcing-typesec.md](https://github.com/querygraph/typesec/blob/main/docs/blog/announcing-typesec.md).

## LakeCat 0.2 "Lynx": the Iceberg REST catalog boundary

LakeCat is a Rust-native, Iceberg-compatible REST catalog. It speaks the
standard protocol — pyiceberg, Spark, and Trino talk to it unchanged — but keeps
the boundary deliberately thin: identity, tenancy, metadata-pointer state, and
policy gates live here, while the reusable engines (Sail for format and scan,
Grust for graph, TypeSec for governance) stay shared. Underneath is a durable
spine on Turso, where every commit is *one transaction* that moves the metadata
pointer via compare-and-swap and writes an audit event, a transactional-outbox
lineage row, and an idempotency record atomically with the table change.

Lynx moves that spine to **Turso MVCC** (`journal_mode = mvcc` + `BEGIN
CONCURRENT`): commits to different tables run truly concurrently, while a
same-table race converges to exactly one winner — no global write lock, no
`database is locked`. Lineage drains from the outbox as OpenLineage events only
after the catalog transaction commits, so it reflects committed state rather than
a handler's best effort. Full post:
[github.com/querygraph/lakecat/blob/main/docs/blog/announcing-lakecat.md](https://github.com/querygraph/lakecat/blob/main/docs/blog/announcing-lakecat.md).

## How they compose in QueryGraph

QueryGraph wires these into a single Rust navigator. **Sail** is the warehouse
that executes the lakehouse and keeps audit data queryable. **Grust "Crab"**
gives the navigator a graph of meaning — dataset contains file, field maps to
concept, policy targets asset, run consumed input — and now a Cypher/GQL way to
ask. **TypeSec "Murano"** turns DIDs and ODRL policy into typed capabilities and
signs every agent interaction, so a compartmentalized agent hierarchy can share
summaries without sharing raw permissions. **LakeCat "Lynx"** is the catalog
boundary: QueryGraph accepts its bootstrap bundles and import plans as *proof* —
table, view, graph, lineage, and receipt-chain hashes that must agree — and
validates the Grust graph shape before building the next agent context. Each run
leaves **OpenLineage** events in Sail and a compact **DID** attestation root.

The result is the architecture QueryGraph has argued for all along: not a single
unrestricted prompt over a warehouse, but a typed, permissioned, lineage-aware
navigator over a semantic lakehouse — now standing on Grust 0.11 "Crab," TypeSec
0.10 "Murano," and LakeCat 0.2 "Lynx." It is built in the open. Kick the tires
and tell us what's missing.
