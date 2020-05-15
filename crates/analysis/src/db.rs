use anyhow::Result;
use lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Range, Url};

use crate::change::{AnalysisChange, RootChange};
use crate::config::Config;
use crate::utils::location::File;
use dialects::dfinance;
use dialects::errors::{CompilerError, CompilerErrorPart};
use utils::{FilePath, FilesSourceText};

#[derive(Debug)]
pub struct FileDiagnostic {
    pub fpath: FilePath,
    pub diagnostic: Option<Diagnostic>,
}

impl FileDiagnostic {
    pub fn new(fpath: FilePath, diagnostic: Diagnostic) -> FileDiagnostic {
        FileDiagnostic {
            fpath,
            diagnostic: Some(diagnostic),
        }
    }

    pub fn new_empty(fpath: FilePath) -> FileDiagnostic {
        FileDiagnostic {
            fpath,
            diagnostic: None,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RootDatabase {
    pub config: Config,
    pub available_files: FilesSourceText,
}

impl RootDatabase {
    pub fn module_files(&self) -> FilesSourceText {
        self.available_files
            .clone()
            .into_iter()
            .filter(|(f, _)| self.is_fpath_for_a_module(f))
            .collect()
    }

    pub fn sender_address(&self) -> [u8; dfinance::types::AccountAddress::LENGTH] {
        self.config.sender_address
    }

    pub fn apply_change(&mut self, change: AnalysisChange) {
        if let Some(config) = change.config_changed {
            self.config = config;
        }
        for root_change in change.tracked_files_changed {
            match root_change {
                RootChange::AddFile(fpath, text) => {
                    log::info!("AddFile: {:?}", fpath);
                    self.available_files.insert(fpath, text);
                }
                RootChange::ChangeFile(fpath, text) => {
                    log::info!("ChangeFile: {:?}", fpath);
                    self.available_files.insert(fpath, text);
                }
                RootChange::RemoveFile(fpath) => {
                    if !self.available_files.contains_key(fpath) {
                        log::warn!("RemoveFile: file {:?} does not exist", fpath);
                    }
                    log::info!("RemoveFile: {:?}", fpath);
                    self.available_files.remove(fpath);
                }
            }
        }
    }

    fn comp_location_to_range(&self, loc: &dialects::errors::Location) -> Result<Range> {
        let file = loc.fpath;
        let text = match self.available_files.get(file) {
            Some(text) => text.clone(),
            None => {
                anyhow::bail!(
                    "File {:?} is not present in the available files {:#?}",
                    file,
                    &self.available_files.keys()
                );
            }
        };
        let file = File::new(text);
        let start_pos = file.position(loc.span.0).unwrap();
        let end_pos = file.position(loc.span.1).unwrap();
        Ok(Range::new(start_pos, end_pos))
    }

    pub fn compiler_error_into_diagnostic(&self, error: CompilerError) -> Result<FileDiagnostic> {
        assert!(!error.parts.is_empty(), "No parts in CompilerError");

        let CompilerErrorPart {
            location: prim_location,
            message,
        } = error.parts[0].to_owned();
        let mut diagnostic = {
            let range = self.comp_location_to_range(&prim_location)?;
            Diagnostic::new_simple(range, message)
        };

        // first error is an actual one, others are related info
        if error.parts.len() > 1 {
            let mut related_info = vec![];
            for CompilerErrorPart { location, message } in error.parts[1..].iter() {
                let range = self.comp_location_to_range(location)?;
                let related_fpath = location.fpath;
                let file_uri = Url::from_file_path(related_fpath)
                    .unwrap_or_else(|_| panic!("Cannot build Url from path {:?}", related_fpath));

                let related_info_item = DiagnosticRelatedInformation {
                    location: Location::new(file_uri, range),
                    message: message.to_string(),
                };
                related_info.push(related_info_item);
            }
            diagnostic.related_information = Some(related_info)
        }
        Ok(FileDiagnostic::new(prim_location.fpath, diagnostic))
    }

    fn is_fpath_for_a_module(&self, fpath: FilePath) -> bool {
        for module_folder in self.config.module_folders.iter() {
            if fpath.starts_with(module_folder.to_str().unwrap()) {
                return true;
            }
        }
        false
    }
}
