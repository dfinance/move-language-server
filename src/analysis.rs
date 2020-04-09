use move_lang::errors::FilesSourceText;

use crate::compiler::utils::leak_str;

#[derive(Default, Debug)]
pub struct AnalysisChange {
    files_changed: Vec<(&'static str, String)>,
}

impl AnalysisChange {
    pub fn new() -> Self {
        AnalysisChange::default()
    }

    pub fn change_file(&mut self, fname: &'static str, new_text: String) {
        let canonical_fname = leak_str(std::fs::canonicalize(fname).unwrap().to_str().unwrap());
        self.files_changed.push((&canonical_fname, new_text))
    }
}

#[derive(Debug, Default)]
pub struct Analysis {
    available_module_files: FilesSourceText,
}

impl Analysis {
    pub fn available_module_files(&self) -> &FilesSourceText {
        &self.available_module_files
    }

    pub fn apply_change(&mut self, change: AnalysisChange) {
        for (fname, new_text) in change.files_changed {
            self.available_module_files.insert(fname, new_text);
        }
    }
}
