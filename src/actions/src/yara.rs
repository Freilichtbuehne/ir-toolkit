use super::{error_result, ActionOptions, ActionResult};
use config::workflow::YaraAttributes;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use log::{debug, error};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::Write,
    fs::File,
    io::BufWriter,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use storage::FileProcessor;
use utils::misc::get_files_by_pattern;
use yara::{Compiler, Rules};

#[derive(Serialize, Deserialize)]
pub struct FileScanResult {
    pub original_path: PathBuf,
    pub indentifier: String,
    pub namespace: String,
    pub error: Option<String>,
}

fn compile_yara_rules(
    rules_paths: &[PathBuf],
    pb: &ProgressBar,
) -> Result<Rules, Box<dyn std::error::Error>> {
    let mut compiler = Compiler::new()?;
    for path in rules_paths {
        compiler = compiler.add_rules_file(path)?;
        pb.inc(1);
    }
    let rules = compiler.compile_rules()?;
    Ok(rules)
}

fn scan_files_with_rules<'a>(
    rules: &'a Rules,
    files: &'a [PathBuf],
    timeout: i32,
    pb: &'a ProgressBar,
    total_hits: &AtomicUsize,
    total_errors: &AtomicUsize,
) -> Vec<FileScanResult> {
    // Iterate over files and scan them with the rules
    let mut results = Vec::new();

    for file in files {
        pb.set_message(format!(
            "Matches: {} Errors: {}",
            total_hits.load(Ordering::Relaxed),
            total_errors.load(Ordering::Relaxed)
        ));

        let result = match rules.scan_file(file, timeout) {
            Ok(result) => result,
            Err(e) => {
                //TODO: fix
                //error!("Error scanning file {}: {}", file.to_string_lossy(), e);
                pb.inc(1);

                results.push(FileScanResult {
                    original_path: file.clone(),
                    indentifier: "".to_string(),
                    namespace: "".to_string(),
                    error: Some(e.to_string()),
                });
                total_errors.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };
        pb.inc(1);

        for match_ in result {
            let result = FileScanResult {
                original_path: file.clone(),
                indentifier: match_.identifier.to_string(),
                namespace: match_.namespace.to_string(),
                error: None,
            };
            total_hits.fetch_add(1, Ordering::Relaxed);
            results.push(result);
        }
    }

    results
}

pub struct Yara {}

impl Yara {
    pub fn run(
        scan: YaraAttributes,
        options: ActionOptions,
        out_file: PathBuf,
        file_processor: &mut FileProcessor,
        custom_files_dir: &PathBuf,
    ) -> ActionResult {
        // initialize csv writer
        let metadata_file = match File::create(&out_file) {
            Ok(file) => file,
            Err(e) => {
                return error_result!(format!("Failed to create metadata file: {}", e));
            }
        };
        let metadata_file = BufWriter::new(metadata_file);

        let mut csv_writer = {
            let writer = csv::Writer::from_writer(metadata_file);
            Some(writer)
        };

        // Step 1: Split pattern string into Vec<String>
        let files_to_scan_patterns = scan.files_to_scan.split('\n').collect::<Vec<&str>>();
        let rules_paths_patterns = scan.rules_paths.split('\n').collect::<Vec<&str>>();

        // Step 2: Check if rule paths are relative or absolute
        let rules_paths_patterns: Vec<String> = rules_paths_patterns
            .iter()
            .map(|pattern| {
                if PathBuf::from(pattern).is_absolute() {
                    pattern.to_string()
                } else {
                    custom_files_dir.join(pattern).to_string_lossy().to_string()
                }
            })
            .collect();

        // Step 3: Get all unique files and rules paths matching the patterns
        let files_to_scan: HashSet<PathBuf> = files_to_scan_patterns
            .iter()
            .flat_map(|pattern| get_files_by_pattern(pattern, false).unwrap_or_default())
            .collect();

        let rules_paths: HashSet<PathBuf> = rules_paths_patterns
            .iter()
            .flat_map(|pattern| get_files_by_pattern(pattern, false).unwrap_or_default())
            .collect();

        let files_to_scan: Vec<PathBuf> = files_to_scan.into_iter().collect();
        let rules_paths: Vec<PathBuf> = rules_paths.into_iter().collect();

        // Both files_to_scan and rules should have at least one element
        if files_to_scan.is_empty() {
            return error_result!("No files to scan provided", options.start_time);
        }
        if rules_paths.is_empty() {
            return error_result!("No rules provided", options.start_time);
        }

        // Step 4: Configure rayon with the number of threads
        rayon::ThreadPoolBuilder::new()
            .num_threads(scan.num_threads as usize)
            .build_global()
            .unwrap();

        // Progress bar setup
        let m = MultiProgress::new();

        debug!(
            "Scanning {} files with {} rules",
            files_to_scan.len(),
            rules_paths.len()
        );

        let rules_pb = m.add(ProgressBar::new(rules_paths.len() as u64));
        rules_pb.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}",
            )
            .unwrap()
            .progress_chars("=>-"),
        );
        rules_pb.set_message("Compiling rules");

        let files_pb = m.add(ProgressBar::new(files_to_scan.len() as u64));
        files_pb.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg} (ETA: {eta})",
            )
            .unwrap()
            .progress_chars("=>-")
            .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
                write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
            }),
        );
        files_pb.set_message("Scanning files");

        // Step 5: Scan files in batches
        let file_batch_size = 500;
        let rule_batch_size = 500;
        let total_hits = AtomicUsize::new(0);
        let total_errors = AtomicUsize::new(0);

        let scan_results: Vec<FileScanResult> = rules_paths
            .par_chunks(rule_batch_size)
            .flat_map(
                |rules_chunk| match compile_yara_rules(rules_chunk, &rules_pb) {
                    Ok(rules) => {
                        files_pb.reset();
                        let chunk_results: Vec<FileScanResult> = files_to_scan
                            .par_chunks(file_batch_size)
                            .flat_map(|files_chunk| {
                                let results = scan_files_with_rules(
                                    &rules,
                                    files_chunk,
                                    scan.scan_timeout,
                                    &files_pb,
                                    &total_hits,
                                    &total_errors,
                                );
                                results
                            })
                            .collect();
                        files_pb.finish_and_clear();
                        chunk_results
                    }
                    Err(e) => {
                        error!("Failed to compile YARA rules: {}", e);
                        Vec::new()
                    }
                },
            )
            .collect();

        // Step 6: Write scan results to the metadata file
        let mut already_stored: HashMap<String, bool> = HashMap::new();

        for result in &scan_results {
            if let Some(ref mut writer) = csv_writer {
                writer.serialize(result).unwrap();
            }

            // Check if the file has already been stored
            let original_path_str = result.original_path.to_string_lossy().to_string();
            if already_stored.contains_key(&original_path_str) {
                continue;
            }

            // Add to file processor if store_on_match is true and no errors
            if scan.store_on_match && result.error.is_none() {
                match file_processor.store(
                    &result.original_path,
                    Some("Matched by YARA: Access time may have changed".to_string()),
                ) {
                    Ok(_) => (),
                    Err(e) => error!("Error storing file: {}", e),
                }
            }

            // Add to already_stored
            already_stored.insert(original_path_str, true);
        }

        ActionResult {
            success: true,
            exit_code: Some(0),
            execution_time: options.start_time.elapsed(),
            error_message: None,
            parallel: false,
            finished: true,
        }
    }
}
