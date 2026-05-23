# QueryGraph

QueryGraph is a Rust implementation sketch for the AI Navigator semantic layer. It turns dataset-facing metadata into a governed JSON-LD bundle with four explicit layers:

1. Semantic Croissant: ML-ready dataset metadata, file objects, record sets, and fields.
2. CDIF: cross-domain FAIR discovery, access, vocabulary, integration, and universal metadata.
3. DID: decentralized identity for agent and service attribution.
4. ODRL: machine-actionable rights, permissions, prohibitions, and constraints.

The design is informed by QueryGraph.ai’s graph/search direction, AgStack Pale Fire’s knowledge-graph search architecture, CODATA CDIF profiles, Croissant Toolkit skills, CODATA local-agent patterns, The Minority Report’s multilingual controlled-vocabulary workflow, and ODRL/DID attribution patterns for local AI services.

## Architecture

```text
src/
  croissant.rs   Dataset, FileObject, RecordSet, Field -> Croissant JSON-LD
  cdif.rs        CDIF profile projection over a Croissant dataset
  did.rs         Deterministic local did:oyd identity document
  odrl.rs        Policy, permissions, prohibitions, and access checks
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
