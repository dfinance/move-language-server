use crate::inner::db::{RootDatabase, FileDiagnostic};
use lang::compiler::check_with_compiler;
use dialects::file::{read_move_files, MoveFile};

#[derive(Debug)]
pub struct Analysis {
    db: RootDatabase,
}

impl Analysis {
    pub fn new(db: RootDatabase) -> Analysis {
        Analysis { db }
    }

    pub fn db(&self) -> &RootDatabase {
        &self.db
    }

    pub fn check_file_with_compiler(
        &self,
        fpath: &'static str,
        text: &str,
    ) -> Option<FileDiagnostic> {
        match self.check_file_with_compiler_inner(fpath, text) {
            Ok(_) => None,
            Err(mut ds) => Some(ds.remove(0)),
        }
    }

    fn check_file_with_compiler_inner(
        &self,
        current_fpath: &'static str,
        current_text: &str,
    ) -> Result<(), Vec<FileDiagnostic>> {
        let deps: Vec<MoveFile> = self
            .read_stdlib_files()
            .into_iter()
            .chain(self.db.module_files().into_iter())
            .filter(|(fpath, _)| *fpath != current_fpath)
            .collect();

        let current_file = MoveFile::new(current_fpath, current_text.to_string());
        check_with_compiler(self.db.config.dialect().as_ref(), &current_file, deps, self.db.config.sender())
            .map_err(|errors| {
                errors
                    .into_iter()
                    .map(
                        |err| match self.db.compiler_error_into_diagnostic(err.clone()) {
                            Ok(d) => d,
                            Err(error) => panic!(
                                "While converting {:#?} into Diagnostic, error occurred: {:?}",
                                err,
                                error.to_string()
                            ),
                        },
                    )
                    .collect()
            })
    }

    fn read_stdlib_files(&self) -> Vec<MoveFile> {
        self.db
            .config
            .stdlib_folder
            .as_ref()
            .map(|folder| read_move_files(folder.as_path()))
            .unwrap_or_else(Vec::new)
    }
}
