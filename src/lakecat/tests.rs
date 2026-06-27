use super::*;
use super::verify::content_hash_json;
use serde_json::json;

#[test]
fn verifies_lakecat_bootstrap_manifest_hashes() {
    let croissant = json!({"@type":"cr:Dataset","name":"events"});
    let cdif = json!({"@type":"dcat:Dataset","dct:title":"events"});
    let osi = json!({"semantic_model":{"name":"events"}});
    let odrl = json!({"@type":"odrl:Policy","@id":"events#odrl"});
    let open_lineage = json!({"eventType":"COMPLETE"});
    let table = LakeCatTableProjection {
        ident: json!({
            "warehouse": "local",
            "namespace": ["default"],
            "name": "events"
        }),
        stable_id: "lakecat:table:local:default:events".to_string(),
        location: "file:///tmp/events".to_string(),
        metadata_location: Some("file:///tmp/events/metadata/00000.json".to_string()),
        version: 0,
        format_version: Some(3),
        croissant: croissant.clone(),
        cdif: cdif.clone(),
        osi: osi.clone(),
        odrl: odrl.clone(),
        policy_bindings: Vec::new(),
    };
    let bundle = LakeCatBootstrapBundle {
        warehouse: "local".to_string(),
        bundle_hash: "pending".to_string(),
        manifest: LakeCatBootstrapManifest {
            schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
            producer: "https://querygraph.ai/lakecat".to_string(),
            standards: vec![
                "Croissant".to_string(),
                "CDIF".to_string(),
                "OSI".to_string(),
                "ODRL".to_string(),
                "OpenLineage".to_string(),
            ],
            table_artifacts: vec![LakeCatTableArtifactHashes {
                stable_id: table.stable_id.clone(),
                croissant_hash: content_hash_json(&croissant).unwrap(),
                cdif_hash: content_hash_json(&cdif).unwrap(),
                osi_hash: content_hash_json(&osi).unwrap(),
                odrl_hash: content_hash_json(&odrl).unwrap(),
                policy_bindings_hash: None,
            }],
            view_artifacts: Vec::new(),
            graph_hash: None,
            open_lineage_hash: content_hash_json(&open_lineage).unwrap(),
            querygraph_import: None,
        },
        tables: vec![table],
        views: Vec::new(),
        graph: json!({
            "nodes": [
                {
                    "id": "lakecat:catalog:local",
                    "label": "Catalog",
                    "properties": {"warehouse": "local"}
                },
                {
                    "id": "lakecat:table:local:default:events",
                    "label": "Table",
                    "properties": {"stableId": "lakecat:table:local:default:events"}
                }
            ],
            "edges": [
                {
                    "from": "lakecat:catalog:local",
                    "to": "lakecat:table:local:default:events",
                    "label": "HAS_TABLE"
                }
            ]
        }),
        open_lineage,
    };
    let bundle = LakeCatBootstrapBundle {
        bundle_hash: bundle.computed_bundle_hash().unwrap(),
        ..bundle
    };

    let verification = bundle.verify_manifest().unwrap();
    assert_eq!(verification.warehouse, "local");
    assert_eq!(verification.table_count, 1);
    assert_eq!(
        verification.verified_tables,
        vec!["lakecat:table:local:default:events"]
    );
    assert_eq!(verification.bundle_hash, bundle.bundle_hash);

    let plan = bundle.import_plan().unwrap();
    assert_eq!(plan.graph_nodes, 2);
    assert_eq!(plan.graph_edges, 1);
    // Crab Cypher over the catalog graph: db.labels() and a Table count.
    assert_eq!(
        plan.catalog_labels,
        vec!["Catalog".to_string(), "Table".to_string()]
    );
    assert_eq!(plan.table_count, 1);
    assert_eq!(
        plan.tables[0].stable_id,
        "lakecat:table:local:default:events"
    );
    assert_eq!(plan.tables[0].croissant_name.as_deref(), Some("events"));
}

#[test]
fn rejects_lakecat_bootstrap_manifest_hash_mismatch() {
    let mut bundle = LakeCatBootstrapBundle {
        warehouse: "local".to_string(),
        bundle_hash: "sha256:not-real".to_string(),
        manifest: LakeCatBootstrapManifest {
            schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
            producer: "https://querygraph.ai/lakecat".to_string(),
            standards: vec!["Croissant".to_string()],
            table_artifacts: vec![LakeCatTableArtifactHashes {
                stable_id: "lakecat:table:local:default:events".to_string(),
                croissant_hash: "sha256:not-real".to_string(),
                cdif_hash: content_hash_json(&json!({})).unwrap(),
                osi_hash: content_hash_json(&json!({})).unwrap(),
                odrl_hash: content_hash_json(&json!({})).unwrap(),
                policy_bindings_hash: None,
            }],
            view_artifacts: Vec::new(),
            graph_hash: None,
            open_lineage_hash: content_hash_json(&json!({})).unwrap(),
            querygraph_import: None,
        },
        tables: vec![LakeCatTableProjection {
            ident: json!({}),
            stable_id: "lakecat:table:local:default:events".to_string(),
            location: "file:///tmp/events".to_string(),
            metadata_location: None,
            version: 0,
            format_version: None,
            croissant: json!({"name":"events"}),
            cdif: json!({}),
            osi: json!({}),
            odrl: json!({}),
            policy_bindings: Vec::new(),
        }],
        views: Vec::new(),
        graph: json!({}),
        open_lineage: json!({}),
    };

    let err = bundle.verify_manifest().unwrap_err().to_string();
    assert!(err.contains("Croissant hash mismatch"));

    bundle.manifest.table_artifacts.clear();
    let err = bundle.verify_manifest().unwrap_err().to_string();
    assert!(err.contains("table artifact count"));
}

#[test]
fn rejects_duplicate_lakecat_table_stable_ids() {
    let table = LakeCatTableProjection {
        ident: json!({}),
        stable_id: "lakecat:table:local:default:events".to_string(),
        location: "file:///tmp/events".to_string(),
        metadata_location: None,
        version: 0,
        format_version: None,
        croissant: json!({}),
        cdif: json!({}),
        osi: json!({}),
        odrl: json!({}),
        policy_bindings: Vec::new(),
    };
    let bundle = LakeCatBootstrapBundle {
        warehouse: "local".to_string(),
        bundle_hash: "sha256:not-checked-before-identity-validation".to_string(),
        manifest: LakeCatBootstrapManifest {
            schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
            producer: "https://querygraph.ai/lakecat".to_string(),
            standards: Vec::new(),
            table_artifacts: vec![
                LakeCatTableArtifactHashes {
                    stable_id: table.stable_id.clone(),
                    croissant_hash: "sha256:not-checked".to_string(),
                    cdif_hash: "sha256:not-checked".to_string(),
                    osi_hash: "sha256:not-checked".to_string(),
                    odrl_hash: "sha256:not-checked".to_string(),
                    policy_bindings_hash: None,
                },
                LakeCatTableArtifactHashes {
                    stable_id: table.stable_id.clone(),
                    croissant_hash: "sha256:not-checked".to_string(),
                    cdif_hash: "sha256:not-checked".to_string(),
                    osi_hash: "sha256:not-checked".to_string(),
                    odrl_hash: "sha256:not-checked".to_string(),
                    policy_bindings_hash: None,
                },
            ],
            view_artifacts: Vec::new(),
            graph_hash: None,
            open_lineage_hash: "sha256:not-checked".to_string(),
            querygraph_import: None,
        },
        tables: vec![table.clone(), table],
        views: Vec::new(),
        graph: json!({}),
        open_lineage: json!({}),
    };

    let err = bundle.verify_manifest().unwrap_err().to_string();
    assert!(err.contains("LakeCat bootstrap table projections must be duplicate-free"));
}

#[test]
fn rejects_lakecat_bootstrap_bundle_hash_mismatch() {
    let croissant = json!({"name":"events"});
    let table = LakeCatTableProjection {
        ident: json!({}),
        stable_id: "lakecat:table:local:default:events".to_string(),
        location: "file:///tmp/events".to_string(),
        metadata_location: None,
        version: 0,
        format_version: None,
        croissant: croissant.clone(),
        cdif: json!({}),
        osi: json!({}),
        odrl: json!({}),
        policy_bindings: Vec::new(),
    };
    let bundle = LakeCatBootstrapBundle {
        warehouse: "local".to_string(),
        bundle_hash: "sha256:bad".to_string(),
        manifest: LakeCatBootstrapManifest {
            schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
            producer: "https://querygraph.ai/lakecat".to_string(),
            standards: vec!["Croissant".to_string()],
            table_artifacts: vec![LakeCatTableArtifactHashes {
                stable_id: table.stable_id.clone(),
                croissant_hash: content_hash_json(&croissant).unwrap(),
                cdif_hash: content_hash_json(&json!({})).unwrap(),
                osi_hash: content_hash_json(&json!({})).unwrap(),
                odrl_hash: content_hash_json(&json!({})).unwrap(),
                policy_bindings_hash: None,
            }],
            view_artifacts: Vec::new(),
            graph_hash: None,
            open_lineage_hash: content_hash_json(&json!({})).unwrap(),
            querygraph_import: None,
        },
        tables: vec![table],
        views: Vec::new(),
        graph: json!({"nodes":[],"edges":[]}),
        open_lineage: json!({}),
    };

    let err = bundle.verify_manifest().unwrap_err().to_string();
    assert!(err.contains("bundle hash mismatch"));
}

#[test]
fn rejects_lakecat_bootstrap_graph_with_invalid_edges() {
    let croissant = json!({"name":"events"});
    let table = LakeCatTableProjection {
        ident: json!({}),
        stable_id: "lakecat:table:local:default:events".to_string(),
        location: "file:///tmp/events".to_string(),
        metadata_location: None,
        version: 0,
        format_version: None,
        croissant: croissant.clone(),
        cdif: json!({}),
        osi: json!({}),
        odrl: json!({}),
        policy_bindings: Vec::new(),
    };
    let bundle = LakeCatBootstrapBundle {
        warehouse: "local".to_string(),
        bundle_hash: "pending".to_string(),
        manifest: LakeCatBootstrapManifest {
            schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
            producer: "https://querygraph.ai/lakecat".to_string(),
            standards: vec!["Croissant".to_string()],
            table_artifacts: vec![LakeCatTableArtifactHashes {
                stable_id: table.stable_id.clone(),
                croissant_hash: content_hash_json(&croissant).unwrap(),
                cdif_hash: content_hash_json(&json!({})).unwrap(),
                osi_hash: content_hash_json(&json!({})).unwrap(),
                odrl_hash: content_hash_json(&json!({})).unwrap(),
                policy_bindings_hash: None,
            }],
            view_artifacts: Vec::new(),
            graph_hash: None,
            open_lineage_hash: content_hash_json(&json!({})).unwrap(),
            querygraph_import: None,
        },
        tables: vec![table],
        views: Vec::new(),
        graph: json!({
            "nodes": [
                {"id": "lakecat:catalog:local", "label": "Catalog"}
            ],
            "edges": [
                {
                    "from": "lakecat:catalog:local",
                    "to": "lakecat:table:missing",
                    "label": "HAS_TABLE"
                }
            ]
        }),
        open_lineage: json!({}),
    };
    let bundle = LakeCatBootstrapBundle {
        bundle_hash: bundle.computed_bundle_hash().unwrap(),
        ..bundle
    };

    let err = bundle.import_plan().unwrap_err().to_string();
    assert!(err.contains("LakeCat bootstrap graph validation failed"));
    assert!(err.contains("edge destination"));
}

#[test]
fn verifies_view_bearing_lakecat_import_contract() {
    let croissant = json!({"name":"events"});
    let cdif = json!({"title":"events"});
    let table_osi = json!({"table":{"name":"events"}});
    let odrl = json!({"@id":"lakecat:policy:events"});
    let view_osi = json!({"view":{"name":"active_customers"}});
    let open_lineage = json!({"eventType":"COMPLETE"});
    let table = LakeCatTableProjection {
        ident: json!({
            "warehouse": "local",
            "namespace": ["default"],
            "name": "events"
        }),
        stable_id: "lakecat:table:local:default:events".to_string(),
        location: "file:///tmp/events".to_string(),
        metadata_location: Some("file:///tmp/events/metadata/00000.json".to_string()),
        version: 1,
        format_version: Some(3),
        croissant: croissant.clone(),
        cdif: cdif.clone(),
        osi: table_osi.clone(),
        odrl: odrl.clone(),
        policy_bindings: vec![json!({"policy-id":"agent-read"})],
    };
    let view = LakeCatViewProjection {
        stable_id: "lakecat:view:local:default:active_customers".to_string(),
        warehouse: "local".to_string(),
        namespace: vec!["default".to_string()],
        name: "active_customers".to_string(),
        view_version: 1,
        sql: "select id from customers where active".to_string(),
        dialect: "sql".to_string(),
        schema_version: 1,
        columns: json!([{"name":"id","data-type":"int"}]),
        properties: json!({}),
        osi: view_osi.clone(),
    };
    let graph = json!({
        "nodes": [
            {"id": "lakecat:catalog", "label": "Catalog"},
            {"id": "lakecat:namespace:local:default", "label": "Namespace"},
            {"id": table.stable_id, "label": "IcebergTable"},
            {"id": view.stable_id, "label": "View"}
        ],
        "edges": [
            {
                "from": "lakecat:catalog",
                "to": "lakecat:namespace:local:default",
                "label": "HAS_NAMESPACE"
            },
            {
                "from": "lakecat:namespace:local:default",
                "to": "lakecat:table:local:default:events",
                "label": "CONTAINS_TABLE"
            },
            {
                "from": "lakecat:namespace:local:default",
                "to": "lakecat:view:local:default:active_customers",
                "label": "CONTAINS_VIEW"
            }
        ]
    });
    let receipt_evidence = vec![LakeCatViewReceiptEvidence {
        stable_id: "lakecat:view:local:default:active_customers".to_string(),
        view_version: 1,
        receipt_hash: "sha256:view-version-receipt".to_string(),
        receipt_chain_hash: "sha256:view-receipt-chain".to_string(),
    }];
    let mut bundle = LakeCatBootstrapBundle {
        warehouse: "local".to_string(),
        bundle_hash: "pending".to_string(),
        manifest: LakeCatBootstrapManifest {
            schema_version: "lakecat.querygraph.bootstrap.v1".to_string(),
            producer: "https://querygraph.ai/lakecat".to_string(),
            standards: vec!["Iceberg REST".to_string(), "OpenLineage".to_string()],
            table_artifacts: vec![LakeCatTableArtifactHashes {
                stable_id: table.stable_id.clone(),
                croissant_hash: content_hash_json(&croissant).unwrap(),
                cdif_hash: content_hash_json(&cdif).unwrap(),
                osi_hash: content_hash_json(&table_osi).unwrap(),
                odrl_hash: content_hash_json(&odrl).unwrap(),
                policy_bindings_hash: Some(
                    content_hash_json(&json!(table.policy_bindings)).unwrap(),
                ),
            }],
            view_artifacts: vec![LakeCatViewArtifactHashes {
                stable_id: view.stable_id.clone(),
                osi_hash: content_hash_json(&view_osi).unwrap(),
            }],
            graph_hash: Some(content_hash_json(&graph).unwrap()),
            open_lineage_hash: content_hash_json(&open_lineage).unwrap(),
            querygraph_import: None,
        },
        tables: vec![table],
        views: vec![view],
        graph,
        open_lineage,
    };
    let receipt_evidence_hash =
        content_hash_json(&serde_json::to_value(&receipt_evidence).unwrap()).unwrap();
    let import = LakeCatQueryGraphImportCompatibility {
        schema_version: "lakecat.querygraph.import-compat.v1".to_string(),
        table_only_bundle_hash: bundle.computed_table_only_bundle_hash().unwrap(),
        view_count: 1,
        graph_hash: bundle.manifest.graph_hash.clone().unwrap(),
        view_receipt_evidence: receipt_evidence,
        view_receipt_evidence_hash: Some(receipt_evidence_hash),
    };
    bundle.manifest.querygraph_import = Some(import);
    bundle.bundle_hash = bundle.computed_bundle_hash().unwrap();

    let plan = bundle.import_plan().unwrap();
    assert_eq!(plan.verification.view_count, 1);
    assert_eq!(plan.views[0].stable_id, bundle.views[0].stable_id);
    assert_eq!(
        plan.verification.querygraph_import_hash.as_deref(),
        Some(
            bundle
                .manifest
                .querygraph_import
                .as_ref()
                .unwrap()
                .table_only_bundle_hash
                .as_str()
        )
    );
}
