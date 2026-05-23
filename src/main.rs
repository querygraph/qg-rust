use anyhow::Result;
use clap::{Parser, Subcommand};
use querygraph::codata::CodataOdrlClient;
use querygraph::{AiNavigator, NavigatorInput};

#[derive(Debug, Parser)]
#[command(name = "querygraph")]
#[command(about = "AI Navigator semantic layer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
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
    }

    Ok(())
}
