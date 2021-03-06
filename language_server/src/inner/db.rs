use anyhow::Result;
use lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Range, Url};
use libra::move_lang::errors::Error;
use libra::move_ir_types::location::Loc;

use serde::export::fmt::Debug;
use serde::export::Formatter;
use std::fmt;
use crate::inner::config::Config;
use crate::inner::change::{AnalysisChange, RootChange};
use std::collections::HashMap;
use lang::compiler::location::File;

pub struct FileDiagnostic {
    pub fpath: String,
    pub diagnostic: Option<Diagnostic>,
}

impl Debug for FileDiagnostic {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut debug_struct = f.debug_struct("FileDiagnostic");

        debug_struct.field("fpath", &self.fpath.to_string());
        if self.diagnostic.is_some() {
            let Diagnostic { range, message, .. } = self.diagnostic.as_ref().unwrap();
            debug_struct
                .field(
                    "range",
                    &format!(
                        "({}, {}) -> ({}, {})",
                        range.start.line,
                        range.start.character,
                        range.end.line,
                        range.end.character,
                    ),
                )
                .field("message", message);
        }
        debug_struct.finish()
    }
}

impl FileDiagnostic {
    pub fn new(fpath: String, diagnostic: Diagnostic) -> FileDiagnostic {
        FileDiagnostic {
            fpath,
            diagnostic: Some(diagnostic),
        }
    }

    pub fn new_empty(fpath: &str) -> FileDiagnostic {
        FileDiagnostic {
            fpath: fpath.to_owned(),
            diagnostic: None,
        }
    }
}

pub struct FilePosition {
    pub fpath: &'static str,
    pub pos: (usize, usize),
}

#[derive(Debug, Default, Clone)]
pub struct RootDatabase {
    pub config: Config,
    pub available_files: HashMap<String, String>,
}

impl RootDatabase {
    pub fn new(config: Config) -> RootDatabase {
        RootDatabase {
            config,
            available_files: Default::default(),
        }
    }

    pub fn module_files(&self) -> HashMap<String, String> {
        self.available_files
            .clone()
            .into_iter()
            .filter(|(f, _)| self.is_fpath_for_a_module(f))
            .collect()
    }

    pub fn apply_change(&mut self, change: AnalysisChange) {
        if let Some(config) = change.config_changed {
            self.config = config;
        }
        for root_change in change.tracked_files_changed {
            match root_change {
                RootChange::AddFile { path, text } => {
                    self.available_files.insert(path, text);
                }
                RootChange::ChangeFile { path, text } => {
                    self.available_files.insert(path, text);
                }
                RootChange::RemoveFile { path } => {
                    if !self.available_files.contains_key(&path) {
                        log::warn!("RemoveFile: file {:?} does not exist", path);
                    }
                    self.available_files.remove(&path);
                }
            }
        }
    }

    fn loc_to_range(&self, loc: &Loc) -> Result<Range> {
        let file = loc.file();
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
        let start_pos = file.position(loc.span().start())?;
        let end_pos = file.position(loc.span().end())?;
        Ok(Range::new(start_pos, end_pos))
    }

    pub fn make_diagnostic(&self, error: Error) -> Result<FileDiagnostic> {
        assert!(!error.is_empty(), "No parts in CompilerError");

        let (loc, msg) = error[0].to_owned();
        let mut diagnostic = {
            let range = self.loc_to_range(&loc)?;
            Diagnostic::new_simple(range, msg)
        };

        // first error is an actual one, others are related info
        if error.len() > 1 {
            let mut related_info = vec![];
            for (loc, msg) in error[1..].iter() {
                let range = self.loc_to_range(loc)?;
                let related_fpath = loc.file();
                let file_uri = Url::from_file_path(related_fpath)
                    .unwrap_or_else(|_| panic!("Cannot build Url from path {:?}", related_fpath));

                let related_info_item = DiagnosticRelatedInformation {
                    location: Location::new(file_uri, range),
                    message: msg.to_string(),
                };
                related_info.push(related_info_item);
            }
            diagnostic.related_information = Some(related_info)
        }

        Ok(FileDiagnostic::new(loc.file().to_owned(), diagnostic))
    }

    fn is_fpath_for_a_module(&self, fpath: &str) -> bool {
        for module_folder in self.config.modules_folders.iter() {
            if let Some(m_folder) = module_folder.to_str() {
                if fpath.starts_with(m_folder) {
                    return true;
                }
            }
        }
        log::info!("{:?} is not a module file (not relative to any module folder), skipping from dependencies", fpath);
        false
    }
}
