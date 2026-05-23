# AI Navigator Semantic Layer

AI Navigator uses the semantic layer as the contract between raw data, knowledge graph indexing, agent execution, and governance.

## Layer 1: Semantic Croissant

Croissant is the dataset-facing layer. QueryGraph uses it to describe datasets, file distributions, record sets, fields, semantic field mappings, creators, licenses, and keywords. This is the layer that lets an ML or agent workflow understand what a dataset contains before loading it.

## Layer 2: CDIF

CDIF is the cross-domain interoperability layer. QueryGraph projects Croissant metadata into the five CDIF concerns:

- Discovery
- Data access and use
- Controlled vocabularies
- Data integration
- Universals such as time, geography, and units

This keeps QueryGraph aligned with FAIR cross-domain reuse rather than a domain-specific catalog only.

## Layer 3: DID

DID is the attribution layer. The implementation creates deterministic `did:oyd`-style identities from local seeds using a SHA-256 multihash encoded as base58btc, matching the `zQm...` shape returned by the CODATA ODRL demo. Every generated bundle can identify the responsible agent or service. A real wallet-backed resolver can replace this local generator without changing the semantic bundle shape.

The CLI can also reproduce the CODATA demo flow:

```bash
cargo run -- anchor-url --url "https://querygraph.ai/resources/"
```

That command calls `https://odrl.dev.codata.org/api/did/create_from_url`, the same endpoint used by the demo SPA at `https://odrl.dev.codata.org/dids?ref=querygraph.ai`.

## Layer 4: ODRL

ODRL is the rights layer. QueryGraph attaches policies directly to dataset targets. Policies can permit public reading with attribution, permit local indexing for a named DID, and prohibit derivative uses such as model training unless a separate agreement exists.

The current ODRL model keeps the core Policy/Rule structure explicit and leaves richer Information Model features, such as duties, consequences, remedies, logical constraints, profile inheritance, and conflict strategies, as the next extension points.

## Resource Spine

The QueryGraph resources page points to the standards and partner materials that define the production target:

- DID Core: identity documents, controllers, services, resolution, and DID URL dereferencing.
- ODRL Information Model: policies over assets with permissions, prohibitions, obligations, duties, constraints, and profile support.
- CODATA ODRL demo: practical DID creation, verification, policy testing, variable DIDs, Croissant DIDs, and group DIDs.
- DDI ISO/PAS 25955:2026: statistical and research metadata interoperability based on DDI Common Core.
- Croissant 1.1: agent-ready dataset metadata with provenance, vocabulary interoperability, and governance.
- CODATA and CR4AI: the cross-domain FAIR and responsible AI ecosystem for this semantic layer.

## Composition

`AiNavigator::build` creates one JSON-LD bundle containing all four layers. This bundle is designed to be:

- stored beside a dataset,
- indexed into a knowledge graph,
- returned by a catalog API,
- inspected by an agent before using a dataset,
- checked by a policy decision point before access or transformation.
