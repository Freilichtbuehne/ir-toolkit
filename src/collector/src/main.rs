use clap::{Arg, Command};
use config::config::{read_config_file, CONFIG_PATH};
use log::{error, info, LevelFilter};
use logging::Logger;
use privileges::{is_elevated, restart_elevated};
use system::SystemVariables;
use utils::misc::exit_after_user_input;
use workflow::handler::WorkflowHandler;

fn main() {
    // Step 1: Initialize system variables
    let system_variables = SystemVariables::new();

    // Step 2: Read the config file
    let config_path = &system_variables.base_path.join(CONFIG_PATH);
    let config = match read_config_file(config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Error reading config file: {}", e);
            return;
        }
    };

    // Step 3: Initialize the logger
    let matches = get_command().get_matches();
    let logger = Logger::init()
        .set_file()
        .set_level(match matches.get_flag("verbose") {
            true => LevelFilter::Debug,
            false => LevelFilter::Info,
        })
        .set_time_config(config.time)
        .apply();

    logger.log_initial_info();
    info!("{}", system_variables);

    // Step 4: Elevate the process
    if config.elevate && !is_elevated() {
        restart_elevated();
    }

    // Step 5: Initialize the workflow handler
    let mut workflow_handler = WorkflowHandler::init(system_variables);
    workflow_handler.run();

    info!("Workflow finished successfully");

    logger.finish();

    // Step 6: Wait for user input
    exit_after_user_input("Press any key to exit...", 0);
}

fn get_command() -> Command {
    Command::new("Collector")
        .version("1.0")
        .about("Runs the defined workflows")
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enables verbose logging")
                .action(clap::ArgAction::SetTrue),
        )
}
