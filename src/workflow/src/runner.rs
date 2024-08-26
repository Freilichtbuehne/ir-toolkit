use actions::{
    binary, command, store, terminal, waiting_result, yara, ActionOptions, ActionResult,
};
use config::workflow::{
    read_workflow_file, ActionType, BinaryAttributes, CommandAttributes, OnError, StoreAttributes,
    TerminalAttributes, WorkflowItem, WorkflowRunner, YaraAttributes,
};
use futures::stream::FuturesUnordered;
use futures::{executor::block_on, StreamExt};
use log::{error, info};
use report::Report;
use std::{error::Error, future::Future, path::PathBuf, pin::Pin};
use storage::FileProcessor;
use system::SystemVariables;
use utils::{misc::wait_for_user_input, sanitize::sanitize_dirname};

#[derive(Debug)]
pub struct Workflow {
    pub runner: WorkflowRunner,
    pub current_step: usize,
}

impl Workflow {
    pub fn init(yaml_path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let runner = read_workflow_file(yaml_path)?;

        Ok(Self {
            runner: runner,
            current_step: 0,
        })
    }

    #[tokio::main]
    pub async fn run(
        &mut self,
        report: &Report,
        system_variables: &SystemVariables,
        file_processor: &mut FileProcessor,
    ) -> Result<(), Box<dyn Error>> {
        let num_steps = self.runner.workflow.len();

        let mut futures: FuturesUnordered<
            Pin<Box<dyn Future<Output = (WorkflowItem, ActionResult)>>>,
        > = FuturesUnordered::new();

        while self.current_step < num_steps {
            let workflow_item = self.runner.workflow[self.current_step].clone();

            let action: &mut config::workflow::Action = match self
                .runner
                .actions
                .iter_mut() // Note: iter_mut to get a mutable reference
                .find(|action| action.name == workflow_item.action)
            {
                Some(action) => action,
                None => {
                    error!("Action not found: {}", workflow_item.action);
                    return Err("Action not found".into());
                }
            };

            let action_name = &action.name;

            let options = ActionOptions {
                timeout: workflow_item.timeout,
                parallel: workflow_item.parallel,
                start_time: std::time::Instant::now(),
            };

            // iteralte over all attributes and replace placeholders with system variables
            action.attributes.replace_vars(&system_variables.as_map());

            //TODO: Normalize paths (e.g. forwards and backwards slashes)
            let result: ActionResult = match action.action_type {
                ActionType::Binary => {
                    // convert action attributes to binary attributes
                    let binary_attributes: BinaryAttributes = action.attributes.clone().into();
                    info!("Running binary action: {}", action_name);

                    // check if log to file is enabled
                    let out_file: Option<PathBuf> = if binary_attributes.log_to_file {
                        let sanitized_name = sanitize_dirname(action_name);
                        Some(
                            report
                                .action_log_dir
                                .join(format!("{}.log", sanitized_name)),
                        )
                    } else {
                        None
                    };

                    let custom_files_dir = system_variables.custom_files_directory.clone();

                    // check if we need to run in parallel
                    // if so, add to the futures and run asynchronously
                    // if not, wait for the result
                    if options.parallel {
                        let cloned_workflow_item = workflow_item.clone();
                        let future: Pin<Box<dyn Future<Output = (WorkflowItem, ActionResult)>>> =
                            Box::pin(async {
                                (
                                    cloned_workflow_item,
                                    binary::Binary::run(
                                        binary_attributes,
                                        options,
                                        out_file,
                                        custom_files_dir,
                                    )
                                    .await,
                                )
                            });
                        futures.push(future);
                        waiting_result!()
                    } else {
                        block_on(binary::Binary::run(
                            binary_attributes,
                            options,
                            out_file,
                            custom_files_dir,
                        ))
                    }
                }
                ActionType::Command => {
                    // convert action attributes to command attributes
                    let command_attributes: CommandAttributes = action.attributes.clone().into();
                    info!("Running command action: {}", action_name);

                    // check if log to file is enabled
                    let out_file: Option<PathBuf> = if command_attributes.log_to_file {
                        let sanitized_name = sanitize_dirname(action_name);
                        Some(
                            report
                                .action_log_dir
                                .join(format!("{}.log", sanitized_name)),
                        )
                    } else {
                        None
                    };

                    // check if we need to run in parallel
                    if options.parallel {
                        let cloned_workflow_item = workflow_item.clone();
                        let future: Pin<Box<dyn Future<Output = (WorkflowItem, ActionResult)>>> =
                            Box::pin(async move {
                                (
                                    cloned_workflow_item,
                                    command::ShellCommand::run(
                                        command_attributes,
                                        options,
                                        out_file,
                                    )
                                    .await,
                                )
                            });
                        futures.push(future);
                        waiting_result!()
                    } else {
                        block_on(command::ShellCommand::run(
                            command_attributes,
                            options,
                            out_file,
                        ))
                    }
                }
                ActionType::Store => {
                    // convert action attributes to store attributes
                    let store_attributes: StoreAttributes = action.attributes.clone().into();
                    info!("Running store action: {}", action_name);

                    store::Store::run(store_attributes, options, file_processor)
                }
                ActionType::Terminal => {
                    // convert action attributes to terminal attributes
                    let terminal_attributes: TerminalAttributes = action.attributes.clone().into();
                    info!("Running terminal action: {}", action_name);

                    // check if transcript is enabled
                    let out_file: Option<PathBuf> = if terminal_attributes.enable_transcript {
                        let sanitized_name = sanitize_dirname(action_name);
                        Some(
                            report
                                .action_log_dir
                                .join(format!("{}_transcript.log", sanitized_name)),
                        )
                    } else {
                        None
                    };

                    // check if we need to run in parallel
                    if options.parallel {
                        let cloned_workflow_item = workflow_item.clone();
                        let future: Pin<Box<dyn Future<Output = (WorkflowItem, ActionResult)>>> =
                            Box::pin(async move {
                                (
                                    cloned_workflow_item,
                                    terminal::Terminal::run(terminal_attributes, options, out_file)
                                        .await,
                                )
                            });
                        futures.push(future);
                        waiting_result!()
                    } else {
                        block_on(terminal::Terminal::run(
                            terminal_attributes,
                            options,
                            out_file,
                        ))
                    }
                }
                ActionType::Yara => {
                    // convert action attributes to yara attributes
                    let yara_attributes: YaraAttributes = action.attributes.clone().into();
                    info!("Running yara action: {}", action_name);

                    // generate csv file name where the results will be stored
                    let out_file = report
                        .action_log_dir
                        .join(format!("{}.csv", sanitize_dirname(action_name)));

                    yara::Yara::run(
                        yara_attributes,
                        options,
                        out_file,
                        file_processor,
                        &system_variables.custom_files_directory,
                    )
                }
            };

            // handle
            match self.handle_result(&result, &workflow_item) {
                Ok(_) => {}
                Err(e) => {
                    error!("Error handling result: {}", e);
                    return Err(e);
                }
            }
        }

        // join all futures
        if futures.len() > 0 {
            info!("Waiting for all remaining processes to finish");
            while let Some((workflow_item, action_result)) = futures.next().await {
                match self.handle_result(&action_result, &workflow_item) {
                    Ok(_) => {}
                    Err(e) => {
                        error!("Error handling result: {}", e);
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_result(
        &mut self,
        result: &ActionResult,
        workflow_item: &config::workflow::WorkflowItem,
    ) -> Result<(), Box<dyn Error>> {
        // the action was run in parallel, we don't need to handle the result yet
        if !result.finished {
            self.current_step += 1;
            return Ok(());
        }

        if result.success {
            info!("Action {:?} succeeded:\n{}", workflow_item.action, &result);
        } else {
            error!("Action {:?} failed:\n{}", workflow_item.action, &result);
        }

        // We don't need to handle the on_error if the action was run in parallel
        if result.parallel {
            self.current_step += 1;
            return Ok(());
        }

        // Handle on_error
        // 1. If no error occurred, continue to the next step
        // 2. If an error occurred and on_error is set to goto, jump to the specified step
        // 3. If an error occurred and on_error is set to abort, stop the workflow
        // 4. If an error occurred and on_error is set to continue, continue to the next step
        match result.success {
            true => {
                self.current_step += 1;
            }
            false => {
                match &workflow_item.on_error {
                    OnError::Goto { goto } => {
                        info!("Action failed, jumping to step: {}", goto);
                        // search for the step with the specified name
                        match self
                            .runner
                            .workflow
                            .iter()
                            .position(|step| step.action == *goto)
                        {
                            Some(index) => {
                                self.current_step = index;
                            }
                            None => {
                                error!("Step {:?} in on_error not found", goto);
                                return Err("Step not found".into());
                            }
                        }
                    }
                    OnError::Abort => {
                        error!("Action failed, aborting workflow");
                        return Err("Aborting workflow".into());
                    }
                    OnError::Continue => {
                        error!("Action failed, continuing to the next step");
                        self.current_step += 1;
                    }
                }
            }
        }

        // Check if we have to wait for keypress to continue
        if workflow_item.continue_after_keypress {
            wait_for_user_input("Press any key to continue with...");
        }

        Ok(())
    }
}
