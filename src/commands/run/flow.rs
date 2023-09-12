use crate::commands::run::action::R;
use crate::db::db_handler::DBHandler;
use crate::http::Api;
use clap::Args;
use crossterm::style::Stylize;

#[derive(Args, Debug, Clone)]
pub struct RunFlowArgs {
    // action name
    name: String,
}

impl RunFlowArgs {
    pub async fn run_flow(
        &self,
        db_handler: &DBHandler,
        requester: &Api<'_>,
    ) -> anyhow::Result<Vec<R>> {
        let maybe_flow = db_handler.get_flow(&self.name).await;
        match maybe_flow {
            Ok(flow) => {
                println!("Running flow {}", flow.name);
                let mut args = flow.de_run_action_args();
                // force rerun flow even if it's already in history and erasing current values
                args.force = true;
                args.run_action(requester).await
            }
            Err(_) => {
                let str = format!("Flow {} not found", self.name).red();
                println!("{}", str);
                anyhow::bail!("Flow not found");
            }
        }
    }
}
