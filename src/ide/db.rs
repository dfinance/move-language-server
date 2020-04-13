use lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Range, Url};
use move_ir_types::location::Loc;
use move_lang::errors::{Error, FilesSourceText};
use move_lang::shared::Address;

use crate::utils::location::File;

pub type FilePath = &'static str;

#[derive(Debug, Default, Clone)]
pub struct RootDatabase {
    pub sender_address: Address,
    pub module_folders: Vec<FilePath>,
    pub all_tracked_files: FilesSourceText,
    pub module_files: FilesSourceText,
}

impl RootDatabase {
    pub fn module_files(&self) -> FilesSourceText {
        let modules = self.all_tracked_files
            .iter()
            .filter(|(f, _)| self.is_fpath_for_a_module(f))
            .map(|(f, t)| (f.clone(), t.clone()))
            .collect();
        dbg!(&self.module_folders);
        modules
    }

    pub fn apply_change(&mut self, change: AnalysisChange) {
        if let Some(address) = change.address_changed {
            self.sender_address = address;
        }
        if let Some(folders) = change.module_folders_changed {
            self.module_folders = folders;
        }

        for root_change in change.tracked_files_changed {
            match root_change {
                RootChange::AddFile(fpath, text) => {
                    log::info!("AddFile: {:?}", fpath);
                    if self.is_fpath_for_a_module(fpath) {
                        self.module_files.insert(fpath, text.clone());
                    }
                    self.all_tracked_files.insert(fpath, text);
                }
                RootChange::ChangeFile(fpath, text) => {
                    log::info!("ChangeFile: {:?}", fpath);
                    if self.is_fpath_for_a_module(fpath) {
                        self.module_files.insert(fpath, text.clone());
                    }
                    self.all_tracked_files.insert(fpath, text);
                }
                RootChange::RemoveFile(fpath) => {
                    if !self.all_tracked_files.contains_key(fpath) {
                        log::warn!("RemoveFile: file {:?} does not exist", fpath);
                    }
                    log::info!("RemoveFile: {:?}", fpath);
                    if self.is_fpath_for_a_module(fpath) {
                        self.module_files.remove(fpath);
                    }
                    self.all_tracked_files.remove(fpath);
                }
            }
        }
    }

    pub fn libra_error_into_diagnostic(&self, error: Error) -> Diagnostic {
        assert!(!error.is_empty(), "Libra's Error is an empty Vec");
        let (primary_loc, primary_message) = error.get(0).unwrap().to_owned();
        let mut diagnostic = {
            let range = self.loc_to_range(primary_loc);
            Diagnostic::new_simple(range, primary_message)
        };
        // first error is an actual one, others are related info
        if error.len() > 1 {
            let mut related_info = vec![];
            for (related_loc, related_message) in error[1..].iter() {
                let range = self.loc_to_range(*related_loc);
                let related_fpath = related_loc.file();
                let file_uri = Url::from_file_path(related_fpath)
                    .unwrap_or_else(|_| panic!("Cannot build Url from path {:?}", related_fpath));

                let related_info_item = DiagnosticRelatedInformation {
                    location: Location::new(file_uri, range),
                    message: related_message.to_string(),
                };
                related_info.push(related_info_item);
            }
            diagnostic.related_information = Some(related_info)
        }
        diagnostic
    }

    fn loc_to_range(&self, loc: Loc) -> Range {
        let text = self.all_tracked_files.get(loc.file()).unwrap().to_owned();
        let file = File::new(text);
        let start_pos = file.position(loc.span().start().to_usize()).unwrap();
        let end_pos = file.position(loc.span().end().to_usize()).unwrap();
        Range::new(start_pos, end_pos)
    }

    fn is_fpath_for_a_module(&self, fpath: FilePath) -> bool {
        for module_folder in self.module_folders.iter() {
            if fpath.starts_with(module_folder) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug)]
pub enum RootChange {
    AddFile(FilePath, String),
    ChangeFile(FilePath, String),
    RemoveFile(FilePath),
}

#[derive(Default, Debug)]
pub struct AnalysisChange {
    address_changed: Option<Address>,
    tracked_files_changed: Vec<RootChange>,
    module_folders_changed: Option<Vec<FilePath>>
}

impl AnalysisChange {
    pub fn new() -> Self {
        AnalysisChange::default()
    }

    pub fn add_file(&mut self, fname: FilePath, text: String) {
        self.tracked_files_changed
            .push(RootChange::AddFile(fname, text));
    }

    pub fn update_file(&mut self, fname: FilePath, text: String) {
        self.tracked_files_changed
            .push(RootChange::ChangeFile(fname, text));
    }

    pub fn remove_file(&mut self, fname: FilePath) {
        self.tracked_files_changed
            .push(RootChange::RemoveFile(fname))
    }

    pub fn change_sender_address(&mut self, new_address: Address) {
        self.address_changed = Some(new_address);
    }

    pub fn change_module_folders(&mut self, folders: Vec<FilePath>) {
        self.module_folders_changed = Some(folders);
    }
}
