# QGLake: The Resilience Desk

QGLake is the end-to-end QueryGraph lakehouse demonstration: a permissioned
AI navigator over enterprise data in Sail, described by Semantic Croissant and
CDIF, governed by RBAC and ODRL, and operated by TypeSec TypeDID agents whose
work is recorded through OpenLineage and DID attestations.

## Story

The Resilience Desk is preparing a summer briefing for a city region. The
supervisor does not ask for a table dump. She asks:

> Where are communities likely to face overlapping energy insecurity, mobility
> disruption, climate-health risk, and fiscal capacity constraints?

The lakehouse contains diverse evidence:

- government finance tables for counties, municipalities, school districts,
  special districts, townships, and states;
- energy access and energy insecurity survey tables;
- dockless transportation, urban form, and pedestrian injury severity data;
- climate and mortality pathway data;
- global party survey data for institutional context;
- CODATA constants as reference data for unit and vocabulary normalization;
- restricted Dataverse assets such as HAALSI, which may be visible as metadata
  but denied at raw-data access time.

Each dataset has two semantic projections:

- Semantic Croissant describes files, record sets, fields, types, semantic
  meanings, and Sail table locations.
- CDIF projects the same material into FAIR discovery, manifest, access,
  integration, access-rights, controlled-vocabulary, universals, and provenance
  profiles.

ODRL policies tell agents what actions they may perform. RBAC decides which
role an agent holds. TypeDID envelopes prove who asked, who answered, what was
delegated, and what payload was signed. OpenLineage records the operational
history in Sail audit tables, while the DID ledger stores compact signed roots.

## Agent Hierarchy

The supervisor coordinates compartmentalized specialists. Specialists do not
share raw rows with each other. They return signed summaries with policy
receipts. A synthesis agent aggregates only those summaries.

| Agent | Role | Raw Scope | Shared Output |
| --- | --- | --- | --- |
| SupervisorAgent | supervisor | delegation graph, summaries | final briefing request and approvals |
| FinanceAgent | finance-specialist | government finance tables | fiscal-capacity summary |
| EnergyAgent | energy-specialist | allowed energy survey fields | energy-burden summary |
| MobilityAgent | mobility-specialist | mobility and injury tables | corridor/mobility-risk summary |
| ClimateHealthAgent | climate-health-specialist | climate-health pathway tables | climate-health risk summary |
| ReferenceAgent | reference-specialist | CODATA/reference vocabularies | unit and vocabulary normalization summary |
| RestrictedDataBroker | restricted-broker | metadata only unless credentialed | denial receipt or metadata-only summary |
| SynthesisAgent | synthesis | signed summaries only | aggregated resilience briefing |

## Permission Model

The demonstration uses a permissioned hierarchy:

- The supervisor may delegate and aggregate, but does not automatically read all
  raw rows.
- Specialists receive granular rights over one compartment.
- The synthesis agent can aggregate summaries, not raw data.
- Restricted datasets can produce signed denial receipts.
- Every request and response is wrapped with TypeDID.

Example permissions:

| Principal | RBAC Action | RBAC Resource | ODRL Action | Result |
| --- | --- | --- | --- | --- |
| FinanceAgent | read | compartment:finance | read | allowed |
| FinanceAgent | export | compartment:finance | use | denied |
| EnergyAgent | summarize | compartment:energy | derive | allowed |
| MobilityAgent | summarize | compartment:mobility | derive | allowed |
| ClimateHealthAgent | summarize | compartment:climate-health | derive | allowed |
| ReferenceAgent | normalize | compartment:reference | read | allowed |
| RestrictedDataBroker | read | compartment:restricted:metadata | read | metadata-only allowed |
| RestrictedDataBroker | read | compartment:restricted:raw | read | denied without credential |
| SynthesisAgent | aggregate | summaries:* | derive | allowed |

## Executable Scenario

Run the story locally:

```bash
cargo run -- qglake-story
```

The command prints a readable briefing. For the full machine-readable report,
run:

```bash
cargo run -- qglake-story --json
```

The full report contains:

- all agent DID identifiers;
- Semantic Croissant and CDIF projections for the scenario datasets;
- ODRL policies and RBAC receipts for each delegated task;
- TypeDID signed request and response envelopes;
- specialist summaries;
- one denied restricted-data receipt;
- aggregated synthesis;
- OpenLineage-style event metadata and DID attestation roots suitable for
  emission to Sail audit tables.

The full lakehouse path can then be paired with the existing commands:

```bash
cargo run -- lakehouse-load --root .querygraph/lakehouse --schema qg_lakehouse
cargo run -- lakehouse-verify --report .querygraph/lakehouse/manifest/load-report.json
cargo run -- lakehouse-validate --report .querygraph/lakehouse/manifest/load-report.json \
  --openlineage-file .querygraph/openlineage/events.jsonl
```

When Sail is running, the live end-to-end path emits OpenLineage into Sail:

```bash
cargo run -- dataverse-e2e \
  --live-sail \
  --sail-endpoint http://127.0.0.1:50051 \
  --openlineage-file .querygraph/openlineage/events.jsonl \
  --did-ledger-file .querygraph/did-ledger/attestations.jsonl
```

## What This Proves

QGLake demonstrates that QueryGraph is not a single unrestricted AI prompt over
a warehouse. It is a typed, permissioned, lineage-aware agent hierarchy over a
semantic lakehouse. Agents can discover rich metadata, request granular access,
summarize within compartments, share signed summaries, aggregate approved
findings, and leave audit evidence without collapsing all enterprise data into
one trust boundary.
