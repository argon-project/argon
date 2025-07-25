//! Unstable interface to the compiler.
//! Do not rely on this interface. It may change anytime.
//! Use the command-line interface instead.

use std::collections::HashMap;
use std::error;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex
};
use tokio::task::JoinSet;
use glob::{
    glob, GlobError, PatternError
};
use log::{
    debug
};
use thiserror;

use crate::frontend::c;
use crate::ir;
use crate::compiler::{
    diagnostics::{self, DiagnosticReporter},
};

pub mod manifest;

pub struct Target {
    name: String,
    files: Vec<String>,
    language: Option<ir::Language>,
    dialect: ir::Dialect,
}

pub struct Product {
    format: String,
    targets: Option<Vec<String>>,
}

pub struct Role {
    label: String
}

pub struct Property {
    label: String
}

pub struct CLanguageSettings {
    pub sema_gen: c::sema_gen::Config,
}

pub struct DoxygenSettings {
    pub commands: Vec<DoxygenCommand>,
}

pub struct DoxygenCommand {
    name: String,
    parameters: Vec<String>,
    actions: HashMap<String, DoxygenAction>,
}

pub struct DoxygenAction {

}

pub struct Config {
    pub config_path: PathBuf,
    pub base_path: PathBuf,

    pub doxygen: DoxygenSettings,
    pub c: CLanguageSettings,

    pub roles: HashMap<String, Role>,
    pub properties: HashMap<String, Property>,
}

#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("The pattern '{1}' in target '{0}' is invalid: {2}")]
    InvalidGlobPattern(String, String, PatternError),

    #[error("The pattern '{1}' in target '{0}' is not resolvable: {2}")]
    Glob(String, String, GlobError),

    #[error("The {0} dialect specified in target {1} is not supported")]
    UnsupportedDialect(ir::Dialect, String),

    #[error(transparent)]
    CAST(#[from] c::ast::ASTError),

    #[error(transparent)]
    CSema(#[from] c::sema_gen::SemaGenError<&'static dyn error::Error>),
}

impl diagnostics::Diagnostic for TargetError {
    fn severity(&self) -> diagnostics::Severity {
        match self {
            Self::CAST(e) => e.severity(),
            Self::CSema(e) => e.severity(),
            _ => diagnostics::Severity::Error,
        }
    }
}

impl Target {
    async fn ingest(
        &self, 
        tasks: &mut JoinSet<Result<(), ()>>, 
        graph: Arc<Mutex<ir::EntryGraph>>,
        config: Arc<Config>,
        diags: &impl DiagnosticReporter
    ) -> Result<(), TargetError> {
        match self.dialect {
            ir::Dialect::Doxygen => {},
            dialect => {
                return Err(TargetError::UnsupportedDialect(
                    dialect, 
                    self.name.clone()
                ));
            }
        }

        let base = config.base_path
            .to_str()
            .map_or(String::new(), |s| 
                s.to_string());

        let pattern_results = self.files.iter().enumerate()
            .filter_map(|(i, f)| glob((base.clone() + f).as_str())
                .map_err(|e| diags.diagnose(TargetError::InvalidGlobPattern(
                        self.name.clone(), f.clone(), e)))
                .ok()
                .map(|paths| (i, paths))
            );

        let mut count = 0;

        for (pattern_ix, paths) in pattern_results {
            for path in paths {
                let Ok(path) = path else {
                    diags.diagnose(TargetError::Glob(
                        self.name.clone(),
                        self.files[pattern_ix].clone(),
                        path.unwrap_err()
                    ));
                    continue;
                };

                count += 1;

                debug!("Target {} has {}", self.name, path.display());
            }

            count += 1;

            let language = self.language.clone();
            let dialect = self.dialect.clone();

            tasks.spawn(async move {
                match language {
                    Some(ir::Language::C) => {
                        debug!("found C source")
                    },
                    _ => {}
                };

                match dialect {
                    ir::Dialect::Doxygen => {

                    },
                    _ => {}
                };

                Ok(())
            });
        }

        Ok(())
    }
}