use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use structopt::StructOpt;

use analysis::utils::io::leaked_fpath;

use dialects::dfinance::{report_errors, FilesSourceText};
use dialects::resources::{ResourceChange, VMStatusVerbose};
use dialects::FilePath;
use move_executor::{executor, io};

#[derive(StructOpt)]
struct Options {
    // required positional
    #[structopt()]
    script: PathBuf,

    #[structopt(short, long)]
    sender: String,

    #[structopt(long)]
    modules: Option<Vec<PathBuf>>,

    #[structopt(long)]
    genesis: Option<PathBuf>,
}

fn parse_genesis_json(fpath: Option<PathBuf>) -> Result<Vec<ResourceChange>> {
    let genesis = match fpath {
        None => vec![],
        Some(fpath) => {
            let text = fs::read_to_string(fpath.clone())?;
            let val = serde_json::from_str(&text)?;
            serde_json::from_value::<Vec<ResourceChange>>(val)
                .with_context(|| format!("{:?} contains invalid genesis data", fpath))?
        }
    };
    Ok(genesis)
}

fn get_file_sources_mapping(
    script: (FilePath, String),
    deps: Vec<(FilePath, String)>,
) -> FilesSourceText {
    let mut mapping = FilesSourceText::with_capacity(deps.len() + 1);
    for (fpath, text) in vec![script].into_iter().chain(deps.into_iter()) {
        mapping.insert(fpath, text);
    }
    mapping
}

fn main() -> Result<()> {
    let options: Options = Options::from_args();

    let script_text = fs::read_to_string(&options.script)?;
    let deps = io::load_module_files(options.modules.unwrap_or_default())?;

    let genesis = parse_genesis_json(options.genesis)?;

    let sender = dialects::dfinance::parse_account_address(&options.sender)?;
    let script_fpath = leaked_fpath(options.script);
    let exec_res =
        executor::compile_and_run((script_fpath, script_text.clone()), &deps, sender, genesis);
    let vm_result = match exec_res {
        Ok(vm_res) => vm_res,
        Err(errors) => {
            let files_mapping = get_file_sources_mapping((script_fpath, script_text), deps);
            report_errors(files_mapping, errors);
        }
    };
    let out = match vm_result {
        Ok(changes) => serde_json::to_string_pretty(&changes).unwrap(),
        Err(vm_status) => {
            let vm_status = VMStatusVerbose::from(vm_status);
            serde_json::to_string_pretty(&vm_status).unwrap()
        }
    };
    println!("{}", out);
    Ok(())
}