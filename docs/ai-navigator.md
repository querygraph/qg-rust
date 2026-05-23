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

DID is the attribution layer. The current implementation creates deterministic `did:oyd`-style identities from local seeds so every generated bundle can identify the responsible agent or service. Later implementations can replace this with a real wallet-backed resolver without changing the semantic bundle shape.

## Layer 4: ODRL

ODRL is the rights layer. QueryGraph attaches policies directly to dataset targets. Policies can permit public reading with attribution, permit local indexing for a named DID, and prohibit derivative uses such as model training unless a separate agreement exists.

## Composition

`AiNavigator::build` creates one JSON-LD bundle containing all four layers. This bundle is designed to be:

- stored beside a dataset,
- indexed into a knowledge graph,
- returned by a catalog API,
- inspected by an agent before using a dataset,
- checked by a policy decision point before access or transformation.
