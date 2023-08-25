mod db;
mod http;
mod json_path;
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new project
    CreateProject(ProjectArgs),
    /// Adds url to a specified project
    AddUrl(ProjectUrls),
    // Get url info for a specified project
    ProjectInfo(ProjectInfoArgs),
    // Add action to a specified url
    AddAction(AddActionArgs),
    // Run action for a specified url
    RunAction(RunActionArgs),
    // delete action for a specified url
    DeleteAction(DeleteActionArgs),
}

#[derive(Args)]
struct ProjectArgs {
    name: String,
}

impl ProjectArgs {
    pub async fn create(&self, db_handler: &db::DBHandler) -> anyhow::Result<()> {
        db_handler.create_project(self.name.as_str()).await?;
        Ok(())
    }
}

#[derive(Args)]
struct ProjectUrls {
    // name of the project
    project_name: String,

    // url of the project with an environement configuration
    #[arg(short, long)]
    url: String,

    // additional configuration for the url
    #[arg(short, long)]
    conf: Vec<String>,

    #[arg(long)]
    env: String, // should be an enum
}

impl ProjectUrls {
    pub async fn add_url_to_project(&self, db_handler: &db::DBHandler) -> anyhow::Result<()> {
        db_handler
            .add_url_to_project(
                self.project_name.as_str(),
                self.url.as_str(),
                &self.conf,
                self.env.as_str(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Args)]
struct ProjectInfoArgs {
    name: Option<String>,
}

impl ProjectInfoArgs {
    pub async fn project_info(&self, db_handler: &db::DBHandler) -> anyhow::Result<()> {
        match self.name.as_ref() {
            Some(name) => db_handler.list_project_url(name.as_str()).await?,
            None => db_handler.list_projects().await?,
        }
        Ok(())
    }
}

#[derive(Args)]
struct AddActionArgs {
    // project name
    project_name: String,

    // name of the action
    #[arg(short, long)]
    name: String,

    // url of the action
    #[arg(short, long)]
    url: String,

    // verb of the action
    #[arg(short, long)]
    verb: String,

    // env of the action
    #[arg(short, long)]
    env: String, // should be an enum
}

impl AddActionArgs {
    pub async fn add_action(&self, db_handler: &db::DBHandler) -> anyhow::Result<()> {
        let body_example = None;
        let response_example = None;
        db_handler
            .upsert_action(
                self.project_name.as_str(),
                self.name.as_str(),
                self.url.as_str(),
                self.verb.as_str(),
                self.env.as_str(),
                &body_example,
                &response_example,
            )
            .await?;
        Ok(())
    }
}

#[derive(Args)]
struct RunActionArgs {
    // project name
    name: String,

    // name of the action
    #[arg(short, long)]
    body: Option<String>,

    //
    #[arg(short, long)]
    header: Vec<String>,

    #[arg(short, long)]
    extract: Option<String>,
}

impl RunActionArgs {
    pub async fn run_action(
        &self,
        db_handler: &db::DBHandler,
        requester: &http::Api,
    ) -> anyhow::Result<()> {
        let action = db_handler.get_action(self.name.as_str()).await?;
        let result = requester
            .fetch(
                &action.full_url(),
                action.api_key.as_str(),
                action.verb.as_str(),
                &self.body,
            )
            .await
            .ok();

        db_handler
            .upsert_action(
                action.project_name.as_str(),
                action.name.as_str(),
                action.url.as_str(),
                action.verb.as_str(),
                action.env.as_str(),
                &self.body,
                &result,
            )
            .await?;
        println!("Full Response: {}", result.clone().expect("no result"));
        if let Some(extract) = &self.extract {
            let extracted = json_path::json_path(result.unwrap().as_str(), extract.as_str());
            println!("Extracted: {}", extracted.unwrap());
        }

        Ok(())
    }
}

#[derive(Args)]
struct DeleteActionArgs {
    name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // init database if needed
    let mut db_handler = db::DBHandler::new();
    db_handler.init_db().await?;

    // init http requester
    let requester = http::Api::new();

    // parse cli args
    let cli: Cli = Cli::parse();
    match &cli.command {
        Commands::CreateProject(project_args) => {
            project_args.create(&db_handler).await?;
        }
        Commands::AddUrl(project_urls) => {
            project_urls.add_url_to_project(&db_handler).await?;
        }
        Commands::ProjectInfo(project_args) => {
            project_args.project_info(&db_handler).await?;
        }
        Commands::AddAction(add_action) => {
            add_action.add_action(&db_handler).await?;
        }
        Commands::RunAction(run_action) => {
            run_action.run_action(&db_handler, &requester).await?;
        }
        Commands::DeleteAction(delete_action) => {
            db_handler
                .delete_action(delete_action.name.as_str())
                .await?;
        }
        _ => println!("Not yet implemented"),
    }
    Ok(())
}
