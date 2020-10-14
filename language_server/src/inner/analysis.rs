use utils::{MoveFile, MoveFilePath};
use crate::inner::db::{RootDatabase, FileDiagnostic};
use lang::compiler::check;
use lang::file::MvFile;
use lang::file;

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

    pub fn check_file_with_compiler(&self, file: MvFile) -> Option<FileDiagnostic> {
        match self.check_file_with_compiler_inner(file) {
            Ok(_) => None,
            Err(mut ds) => Some(ds.remove(0)),
        }
    }

    fn check_file_with_compiler_inner(
        &self,
        current_file: MvFile,
    ) -> Result<(), Vec<FileDiagnostic>> {
        let deps: Vec<MvFile> = self
            .read_stdlib_files()
            .into_iter()
            .chain(
                self.db
                    .module_files()
                    .into_iter()
                    .map(|(name, text)| MvFile::with_content(name, text)),
            )
            .filter(|file| file.name() != current_file.name())
            .collect();

        check(
            self.db.config.dialect().as_ref(),
            current_file,
            deps,
            Some(self.db.config.sender().as_address()),
        )
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

    fn read_stdlib_files(&self) -> Vec<MvFile> {
        self.db
            .config
            .stdlib_folder
            .as_ref()
            .map(|folder| file::load_move_files(&[folder]).unwrap_or_default())
            .unwrap_or_default()
    }
}
