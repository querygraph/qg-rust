use super::*;

use chrono::DateTime;
use lakecat_core::{Namespace, TableIdent, TableName, WarehouseName};
use qglake_bundle::{
    QueryGraphBootstrap, QueryGraphBundleManifest, QueryGraphCatalogGraph, QueryGraphEdge,
    QueryGraphImportCompatibility, QueryGraphNode, QueryGraphTableArtifactHashes,
    QueryGraphTableProjection, QueryGraphViewArtifactHashes, QueryGraphViewProjection,
    QueryGraphViewReceiptEvidence, graph_hash,
};
use serde_json::json;

const TABLE_ID: &str = "lakecat:table:local:default:events";

fn warehouse() -> WarehouseName {
    WarehouseName::new("local").unwrap()
}

fn table_projection(stable_id: &str) -> QueryGraphTableProjection {
    QueryGraphTableProjection {
        ident: TableIdent::new(
            warehouse(),
            Namespace::new(vec!["default".to_string()]).unwrap(),
            TableName::new("events").unwrap(),
        ),
        stable_id: stable_id.to_string(),
        location: "s3://warehouse/events".to_string(),
        metadata_location: Some("s3://warehouse/events/metadata/0.json".to_string()),
        version: 1,
        format_version: Some(2),
        croissant: json!({"@type": "cr:Dataset", "name": "events"}),
        cdif: json!({"@type": "dcat:Dataset", "dct:title": "events"}),
        osi: json!({"semantic_model": {"name": "events"}}),
        odrl: json!({"@type": "odrl:Policy", "@id": "events#odrl"}),
        policy_bindings: vec![],
    }
}

fn view_projection(stable_id: &str) -> QueryGraphViewProjection {
    QueryGraphViewProjection {
        stable_id: stable_id.to_string(),
        warehouse: "local".to_string(),
        namespace: vec!["default".to_string()],
        name: "events_recent".to_string(),
        view_version: 1,
        sql: "SELECT * FROM events".to_string(),
        dialect: "spark".to_string(),
        schema_version: Some(1),
        columns: json!([]),
        properties: json!({}),
        osi: json!({"view": {"name": "events_recent"}}),
    }
}

fn catalog_graph() -> QueryGraphCatalogGraph {
    QueryGraphCatalogGraph {
        nodes: vec![
            QueryGraphNode {
                id: "lakecat:catalog:local".to_string(),
                label: "Catalog".to_string(),
                properties: json!({"warehouse": "local"}),
            },
            QueryGraphNode {
                id: TABLE_ID.to_string(),
                label: "Table".to_string(),
                properties: json!({"version": 3}),
            },
        ],
        edges: vec![QueryGraphEdge {
            from: "lakecat:catalog:local".to_string(),
            to: TABLE_ID.to_string(),
            label: "HAS_TABLE".to_string(),
        }],
    }
}

/// Assemble a hash-valid bootstrap. The bundle hash is recomputed by
/// `with_view_receipt_evidence`, so callers never hand-compute hashes.
fn build_bundle(
    tables: Vec<QueryGraphTableProjection>,
    views: Vec<QueryGraphViewProjection>,
    graph: QueryGraphCatalogGraph,
    evidence: Vec<QueryGraphViewReceiptEvidence>,
) -> QueryGraphBootstrap {
    let wh = warehouse();
    let open_lineage = json!({"eventType": "COMPLETE"});
    let table_artifacts = tables
        .iter()
        .map(|table| QueryGraphTableArtifactHashes::from_table(table).unwrap())
        .collect::<Vec<_>>();
    let view_artifacts = views
        .iter()
        .map(|view| QueryGraphViewArtifactHashes::from_view(view).unwrap())
        .collect::<Vec<_>>();
    let graph_hash = graph_hash(&graph).unwrap();
    let mut manifest =
        QueryGraphBundleManifest::from_hashes(table_artifacts, view_artifacts, graph_hash, &open_lineage)
            .unwrap();
    manifest.querygraph_import = Some(
        QueryGraphImportCompatibility::from_table_only_bundle(
            &wh,
            &manifest,
            &tables,
            &graph,
            &open_lineage,
            views.len(),
        )
        .unwrap(),
    );
    let bootstrap = QueryGraphBootstrap {
        warehouse: wh,
        generated_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        bundle_hash: String::new(),
        manifest,
        tables,
        views,
        graph,
        open_lineage,
    };
    bootstrap.with_view_receipt_evidence(evidence).unwrap()
}

fn view_evidence(stable_id: &str) -> QueryGraphViewReceiptEvidence {
    QueryGraphViewReceiptEvidence {
        stable_id: stable_id.to_string(),
        view_version: 1,
        receipt_hash: "sha256:receipt".to_string(),
        receipt_chain_hash: "sha256:receipt-chain".to_string(),
    }
}

#[test]
fn verifies_qglake_bootstrap_and_builds_cypher_import_plan() {
    let bundle = LakeCatBootstrapBundle(build_bundle(
        vec![table_projection(TABLE_ID)],
        vec![],
        catalog_graph(),
        vec![],
    ));

    let verification = bundle.verify_manifest().unwrap();
    assert_eq!(verification.warehouse, "local");
    assert_eq!(verification.table_count, 1);
    assert_eq!(verification.verified_tables, vec![TABLE_ID.to_string()]);

    let plan = bundle.import_plan().unwrap();
    assert_eq!(plan.graph_nodes, 2);
    assert_eq!(plan.graph_edges, 1);
    // Crab Cypher over the catalog graph: db.labels() and a Table count.
    assert_eq!(
        plan.catalog_labels,
        vec!["Catalog".to_string(), "Table".to_string()]
    );
    assert_eq!(plan.table_count, 1);
    assert_eq!(plan.tables[0].stable_id, TABLE_ID);
    assert_eq!(plan.tables[0].croissant_name.as_deref(), Some("events"));
}

#[test]
fn rejects_manifest_hash_mismatch() {
    let mut bootstrap = build_bundle(
        vec![table_projection(TABLE_ID)],
        vec![],
        catalog_graph(),
        vec![],
    );
    bootstrap.manifest.open_lineage_hash = "sha256:tampered".to_string();

    let err = LakeCatBootstrapBundle(bootstrap)
        .verify_manifest()
        .unwrap_err()
        .to_string();
    assert!(err.contains("OpenLineage hash mismatch"), "{err}");
}

#[test]
fn rejects_bundle_hash_mismatch() {
    let mut bootstrap = build_bundle(
        vec![table_projection(TABLE_ID)],
        vec![],
        catalog_graph(),
        vec![],
    );
    bootstrap.bundle_hash = "sha256:wrong".to_string();

    let err = LakeCatBootstrapBundle(bootstrap)
        .verify_manifest()
        .unwrap_err()
        .to_string();
    assert!(err.contains("bundle hash mismatch"), "{err}");
}

#[test]
fn rejects_duplicate_table_stable_ids() {
    let bootstrap = build_bundle(
        vec![table_projection(TABLE_ID), table_projection(TABLE_ID)],
        vec![],
        catalog_graph(),
        vec![],
    );

    let err = LakeCatBootstrapBundle(bootstrap)
        .verify_manifest()
        .unwrap_err()
        .to_string();
    assert!(err.contains("duplicate-free by stable id"), "{err}");
}

#[test]
fn rejects_catalog_graph_with_invalid_edges() {
    // A graph whose edge points at a node not present in the vertex set. The
    // bundle still hashes/verifies (the graph hash covers exactly these bytes);
    // the dangling edge is caught when the importer loads it through Grust.
    let graph = QueryGraphCatalogGraph {
        nodes: vec![QueryGraphNode {
            id: "lakecat:catalog:local".to_string(),
            label: "Catalog".to_string(),
            properties: json!({}),
        }],
        edges: vec![QueryGraphEdge {
            from: "lakecat:catalog:local".to_string(),
            to: "lakecat:table:missing".to_string(),
            label: "HAS_TABLE".to_string(),
        }],
    };
    let bundle = LakeCatBootstrapBundle(build_bundle(
        vec![table_projection(TABLE_ID)],
        vec![],
        graph,
        vec![],
    ));

    bundle.verify_manifest().expect("bundle itself is hash-valid");
    let err = bundle.import_plan().unwrap_err().to_string();
    assert!(err.contains("is not present in vertices"), "{err}");
}

#[test]
fn verifies_view_bearing_import_contract() {
    let view_id = "lakecat:view:local:default:events_recent";
    let bundle = LakeCatBootstrapBundle(build_bundle(
        vec![table_projection(TABLE_ID)],
        vec![view_projection(view_id)],
        catalog_graph(),
        vec![view_evidence(view_id)],
    ));

    let verification = bundle.verify_manifest().unwrap();
    assert_eq!(verification.view_count, 1);
    assert_eq!(verification.verified_views, vec![view_id.to_string()]);
    assert!(!verification.querygraph_import_hash.is_empty());

    let plan = bundle.import_plan().unwrap();
    assert_eq!(plan.views.len(), 1);
    assert_eq!(plan.views[0].stable_id, view_id);
    assert_eq!(plan.views[0].osi_model.as_deref(), Some("events_recent"));
}
