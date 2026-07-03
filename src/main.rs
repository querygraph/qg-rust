use std::fs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use querygraph::agent::{PyTypeDidEnvelope, QueryGraphAgent, call_ollama_via_typedid};
use querygraph::codata::CodataOdrlClient;
use querygraph::dataverse::{DataverseClient, sample_datasets};
use querygraph::lakecat::LakeCatBootstrapBundle;
use querygraph::lakehouse::{
    DEFAULT_SCHEMA, LakehouseLoadOptions, load_default_lakehouse, report_summary,
    verify_lakehouse_report,
};
use querygraph::lineage::{
    LineageAttestation, OpenLineageRunEvent, append_did_ledger_attestation, bundle_hash,
    emit_openlineage_http, emit_openlineage_jsonl, emit_openlineage_sail,
};
use querygraph::osi::OsiDocument;
use querygraph::qglake::{render_qglake_story, run_qglake_story};
use querygraph::sail::LocalSailLakehouse;
use querygraph::validation::validate_lakehouse_semantics;
use querygraph::{AiNavigator, NavigatorInput};

#[derive(Debug, Parser)]
#[command(name = "querygraph")]
#[command(about = "AI Navigator semantic layer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Commands {
    /// Build a four-layer semantic bundle: Croissant, CDIF, DID, and ODRL.
    Navigator {
        #[arg(long)]
        dataset_name: String,
        #[arg(long)]
        description: String,
        #[arg(long)]
        landing_page: String,
        #[arg(long)]
        data_url: String,
        #[arg(long, default_value = "QueryGraph")]
        creator: String,
        #[arg(long, default_value = "AI Navigator")]
        agent_name: String,
    },
    /// Reproduce the CODATA ODRL demo's URL-to-DID anchoring call.
    AnchorUrl {
        #[arg(long, default_value = "https://querygraph.ai/resources/")]
        url: String,
        #[arg(long, default_value = "https://odrl.dev.codata.org")]
        endpoint: String,
    },
    /// Run the Dataverse -> Sail -> CDIF -> TypeDID agent demo.
    DataverseE2e {
        #[arg(long)]
        dataverse_url: Option<String>,
        #[arg(long, default_value = "governed enterprise data")]
        query: String,
        #[arg(long, default_value_t = 5)]
        limit: usize,
        #[arg(long)]
        api_token: Option<String>,
        #[arg(long, default_value = ".querygraph/sail")]
        sail_dir: String,
        #[arg(long)]
        osi_path: Option<String>,
        #[arg(long)]
        live_sail: bool,
        #[arg(long, default_value = "http://127.0.0.1:50051")]
        sail_endpoint: String,
        #[arg(long, default_value = "Which governed datasets are relevant?")]
        question: String,
        #[arg(long)]
        anchor_codata: bool,
        #[arg(long, default_value = "https://odrl.dev.codata.org")]
        codata_endpoint: String,
        #[arg(long)]
        call_ollama: bool,
        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,
        #[arg(long, default_value = "llama3.2")]
        ollama_model: String,
        #[arg(long)]
        openlineage_url: Option<String>,
        #[arg(long)]
        openlineage_file: Option<String>,
        #[arg(long)]
        openlineage_sail: bool,
        #[arg(long, default_value = "qg_audit")]
        openlineage_sail_schema: String,
        #[arg(long)]
        did_ledger_file: Option<String>,
    },
    /// Download the demonstration datasets and materialize typed Sail lakehouse tables.
    LakehouseLoad {
        #[arg(long, default_value = ".querygraph/lakehouse")]
        root: String,
        #[arg(long, default_value = DEFAULT_SCHEMA)]
        schema: String,
        #[arg(long, default_value = "http://127.0.0.1:50051")]
        sail_endpoint: String,
        #[arg(long)]
        max_files_per_dataset: Option<usize>,
        #[arg(long, env = "DATAVERSE_API_TOKEN")]
        api_token: Option<String>,
    },
    /// Verify typed Sail table row counts against a lakehouse load manifest.
    LakehouseVerify {
        #[arg(
            long,
            default_value = ".querygraph/lakehouse/manifest/load-report.json"
        )]
        report: String,
        #[arg(long, default_value = "http://127.0.0.1:50051")]
        sail_endpoint: String,
    },
    /// Validate generated Semantic Croissant, CDIF, and optional OpenLineage JSONL artifacts.
    LakehouseValidate {
        #[arg(
            long,
            default_value = ".querygraph/lakehouse/manifest/load-report.json"
        )]
        report: String,
        #[arg(long)]
        openlineage_file: Option<String>,
    },
    /// Verify a LakeCat QueryGraph bootstrap bundle before graph import.
    LakecatVerify {
        #[arg(long)]
        bundle: String,
    },
    /// Verify a LakeCat bundle and write a QueryGraph import plan.
    LakecatImport {
        #[arg(long)]
        bundle: String,
        #[arg(long)]
        output: String,
    },
    /// Run the QGLake permissioned multi-agent story.
    QglakeStory {
        /// Print the full machine-readable report instead of the readable briefing.
        #[arg(long)]
        json: bool,
    },
    /// Verify a qg-python TypeDID envelope: payload hash and Ed25519 signature.
    VerifyEnvelope {
        /// Path to the envelope JSON ("-" reads stdin).
        #[arg(long)]
        file: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Navigator {
            dataset_name,
            description,
            landing_page,
            data_url,
            creator,
            agent_name,
        } => {
            let output = AiNavigator.build(NavigatorInput {
                dataset_name,
                description,
                landing_page,
                data_url,
                creator,
                agent_name,
            });
            println!("{}", serde_json::to_string_pretty(&output.bundle)?);
        }
        Commands::AnchorUrl { url, endpoint } => {
            let anchored = CodataOdrlClient::new(endpoint).create_did_from_url(&url)?;
            println!("{}", serde_json::to_string_pretty(&anchored)?);
        }
        Commands::DataverseE2e {
            dataverse_url,
            query,
            limit,
            api_token,
            sail_dir,
            osi_path,
            live_sail,
            sail_endpoint,
            question,
            anchor_codata,
            codata_endpoint,
            call_ollama,
            ollama_url,
            ollama_model,
            openlineage_url,
            openlineage_file,
            openlineage_sail,
            openlineage_sail_schema,
            did_ledger_file,
        } => {
            let datasets = if let Some(dataverse_url) = dataverse_url {
                let mut client = DataverseClient::new(dataverse_url);
                if let Some(api_token) = api_token.as_deref() {
                    client = client.with_api_token(api_token);
                }
                client.search_datasets(&query, limit)?
            } else {
                sample_datasets()
            };
            let osi = if let Some(osi_path) = osi_path {
                OsiDocument::from_yaml_file(osi_path)?
            } else {
                OsiDocument::for_dataverse(&datasets)
            };
            let navigator = AiNavigator;
            let first = datasets
                .first()
                .expect("Dataverse search returned no datasets for the demo");
            let output = navigator.build_from_croissant(
                first.to_croissant(),
                first.landing_page.clone(),
                first
                    .files
                    .first()
                    .map(|file| file.download_url.clone())
                    .unwrap_or_else(|| first.landing_page.clone()),
                first
                    .authors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Dataverse".to_string()),
                "QueryGraph Dataverse Navigator",
            );
            let mut sail_report =
                LocalSailLakehouse::new(sail_dir).stage_dataverse_datasets(&datasets)?;
            if live_sail {
                sail_report = tokio::runtime::Runtime::new()?.block_on(
                    sail_report.load_semantic_graph_into_sail(
                        sail_endpoint.clone(),
                        &datasets,
                        &output.bundle,
                        Some(&osi),
                    ),
                )?;
            }
            let codata_anchor = if anchor_codata {
                Some(
                    CodataOdrlClient::new(codata_endpoint)
                        .create_did_from_url(&first.landing_page)?,
                )
            } else {
                None
            };
            let agent = QueryGraphAgent {
                agent_did: output.did.clone(),
                requester_did: querygraph::did::DidDocument::new_oyd(
                    "querygraph-dataverse-e2e-requester",
                    "TypeSec Demo Requester",
                ),
            };
            let rbac = querygraph::rbac::RbacPolicy::new()
                .with_role(querygraph::rbac::RbacRole::new("navigator").allow("answer", "dataset"))
                .assign_role(agent.agent_did.id.clone(), "navigator");
            let mut run = agent.run_dataverse_answer(
                &question,
                &datasets,
                &sail_report,
                &rbac,
                &querygraph::odrl::Policy {
                    id: output.odrl["@id"].as_str().unwrap_or("policy").to_string(),
                    target: output
                        .odrl
                        .get("odrl:target")
                        .and_then(|value| value.as_str())
                        .unwrap_or("dataset")
                        .to_string(),
                    assigner: output.did.id.clone(),
                    permissions: vec![querygraph::odrl::Rule {
                        action: querygraph::odrl::Action::Index,
                        assignee: output.did.id.clone(),
                        constraint: Some("local semantic indexing for AI Navigator".to_string()),
                    }],
                    prohibitions: vec![],
                },
                codata_anchor,
            )?;
            if call_ollama {
                let (reply, ollama_typedid) =
                    call_ollama_via_typedid(&run.prompt, ollama_url, ollama_model)?;
                run.ollama_reply = reply;
                run.ollama_typedid = Some(ollama_typedid);
            }
            let bundle_hash = bundle_hash(&output.bundle);
            let lineage_event = OpenLineageRunEvent::for_dataverse_agent_run(
                &datasets,
                &sail_report,
                &run.request,
                &bundle_hash,
            );
            let lineage_hash = lineage_event.event_hash();
            let lineage_attestation = LineageAttestation::from_event(
                output.did.id.clone(),
                "querygraph.dataverse.e2e",
                &lineage_hash,
            )?;
            let mut lineage_emissions = Vec::new();
            if let Some(path) = openlineage_file {
                lineage_emissions.push(emit_openlineage_jsonl(path, &lineage_event)?);
            }
            if let Some(endpoint) = openlineage_url {
                lineage_emissions.push(emit_openlineage_http(endpoint, &lineage_event)?);
            }
            if openlineage_sail || live_sail {
                lineage_emissions.push(emit_openlineage_sail(
                    sail_endpoint,
                    openlineage_sail_schema,
                    &lineage_event,
                    &lineage_attestation,
                )?);
            }
            let did_ledger_append = if let Some(path) = did_ledger_file {
                Some(append_did_ledger_attestation(path, &lineage_attestation)?)
            } else {
                None
            };
            let report = serde_json::json!({
                "datasets": datasets,
                "osi": osi,
                "sail": sail_report,
                "bundle": output.bundle,
                "agentRun": run,
                "openLineage": {
                    "event": lineage_event,
                    "eventHash": lineage_hash,
                    "emissions": lineage_emissions,
                    "attestation": lineage_attestation,
                    "didLedgerAppend": did_ledger_append
                }
            });
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Commands::LakehouseLoad {
            root,
            schema,
            sail_endpoint,
            max_files_per_dataset,
            api_token,
        } => {
            let report = load_default_lakehouse(LakehouseLoadOptions {
                root: root.into(),
                schema,
                sail_endpoint,
                max_files_per_dataset,
                api_token,
            })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report_summary(&report))?
            );
        }
        Commands::LakehouseVerify {
            report,
            sail_endpoint,
        } => {
            let verification = verify_lakehouse_report(report, sail_endpoint)?;
            println!("{}", serde_json::to_string_pretty(&verification)?);
        }
        Commands::LakehouseValidate {
            report,
            openlineage_file,
        } => {
            let validation = validate_lakehouse_semantics(report, openlineage_file)?;
            println!("{}", serde_json::to_string_pretty(&validation)?);
            if !validation.ok() {
                std::process::exit(1);
            }
        }
        Commands::LakecatVerify { bundle } => {
            let bundle = LakeCatBootstrapBundle::from_path(bundle)?;
            let verification = bundle.verify_manifest()?;
            println!("{}", serde_json::to_string_pretty(&verification)?);
        }
        Commands::LakecatImport { bundle, output } => {
            let bundle = LakeCatBootstrapBundle::from_path(bundle)?;
            let plan = bundle.import_plan()?;
            if let Some(parent) = std::path::Path::new(&output).parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output, serde_json::to_vec_pretty(&plan)?)?;
            println!("{}", serde_json::to_string_pretty(&plan.verification)?);
        }
        Commands::QglakeStory { json } => {
            let report = run_qglake_story()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", render_qglake_story(&report));
            }
        }
        Commands::VerifyEnvelope { file } => {
            let json = if file == "-" {
                std::io::read_to_string(std::io::stdin())?
            } else {
                fs::read_to_string(&file)?
            };
            let report = PyTypeDidEnvelope::from_json(&json)?.verify();
            println!("{}", serde_json::to_string_pretty(&report)?);
            if !report.signature_valid {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
