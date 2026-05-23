# QueryGraph

QueryGraph is a Rust implementation sketch for the AI Navigator semantic layer. It turns dataset-facing metadata into a governed JSON-LD bundle with four explicit layers:

1. Semantic Croissant: ML-ready dataset metadata, file objects, record sets, and fields.
2. CDIF: cross-domain FAIR discovery, access, vocabulary, integration, and universal metadata.
3. DID: decentralized identity for agent and service attribution.
4. ODRL: machine-actionable rights, permissions, prohibitions, and constraints.

The design is informed by QueryGraph.ai’s graph/search direction, AgStack Pale Fire’s knowledge-graph search architecture, CODATA CDIF profiles, Croissant Toolkit skills, CODATA local-agent patterns, The Minority Report’s multilingual controlled-vocabulary workflow, and ODRL/DID attribution patterns for local AI services.

The QueryGraph resources page adds the standards spine:

- W3C DID Core for DID documents, controllers, services, and resolution.
- W3C ODRL Information Model 2.2 for permissions, prohibitions, duties, constraints, and policy profiles.
- CODATA ODRL demo for URL-to-DID anchoring at `/api/did/create_from_url`.
- DDI ISO/PAS 25955:2026 for research/statistical metadata interoperability.
- MLCommons Croissant 1.1 for agent-ready provenance, vocabulary interoperability, and governance.

## Architecture

```text
src/
  croissant.rs   Dataset, FileObject, RecordSet, Field -> Croissant JSON-LD
  cdif.rs        CDIF profile projection over a Croissant dataset
  did.rs         Deterministic local did:oyd identity document
  odrl.rs        Policy, permissions, prohibitions, and access checks
  codata.rs      Client for CODATA ODRL demo DID anchoring APIs
  navigator.rs   AI Navigator composition pipeline
  main.rs        CLI entry point
```

The first product boundary is intentionally narrow: build a semantic bundle that can be indexed by a knowledge graph, served by an agent, or attached to a dataset catalog. Query/search, graph storage, and external policy resolution should build on this library instead of being embedded directly in it.

## Run

```bash
cargo run -- navigator \
  --dataset-name "Hazard vocabulary" \
  --description "Controlled vocabulary with multilingual technical terms" \
  --landing-page "https://querygraph.ai/datasets/hazards" \
  --data-url "https://querygraph.ai/datasets/hazards.csv"
```

Reproduce the CODATA ODRL demo anchoring behavior for QueryGraph resources:

```bash
cargo run -- anchor-url --url "https://querygraph.ai/resources/"
```

This calls `https://odrl.dev.codata.org/api/did/create_from_url` and returns the `did:oyd` identifier plus the stored payload reported by the demo service.

## Test

```bash
cargo test
```

## Next Milestones

- Add JSON Schema validation for incoming metadata.
- Add parsers for Croissant JSON-LD and CDIF JSON-LD back into typed Rust structs.
- Add policy decision points for ODRL constraints beyond simple action matching.
- Add graph export adapters for Neo4j, RDF/Turtle, and Qdrant payloads.
- Add a local-agent integration surface compatible with MCP or agents-cli style runners.
