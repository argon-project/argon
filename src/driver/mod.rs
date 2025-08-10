//! Unstable interface to the compiler.
//! Do not rely on this interface. It may change anytime.
//! Use the command-line interface instead.

use std::collections::HashMap;
use std::error;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex
};
use tokio::task::{
    JoinError,
    JoinSet,
};
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
    pub name: String,
    pub files: Vec<String>,
    pub language: Option<ir::Language>,
    pub dialect: ir::Dialect,
}

pub struct Product {
    pub format: String,
    pub targets: Option<Vec<String>>,
}

pub struct Role {
    pub label: String
}

pub struct Attribute {
    pub label: String
}

pub struct CLanguageSettings {
    pub sema_gen: c::sema_gen::Config,
}

pub struct DoxygenSettings {
    pub commands: Vec<DoxygenCommand>,
}

pub struct DoxygenCommand {
    pub name: String,
    pub parameters: Vec<String>,
    pub actions: HashMap<String, DoxygenAction>,
}

pub struct DoxygenAction {

}

pub struct Config {
    pub base_path: PathBuf,
    pub doxygen: DoxygenSettings,
    pub c: CLanguageSettings,

    pub roles: HashMap<String, Role>,
    pub attributes: HashMap<String, Attribute>,
}

#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("The pattern '{1}' in target '{0}' is invalid: {2}")]
    InvalidGlobPattern(String, String, PatternError),

    #[error("The pattern '{1}' in target '{0}' is not resolvable: {2}")]
    Glob(String, String, GlobError),

    #[error("The {0} dialect specified in target {1} is not supported")]
    UnsupportedDialect(ir::Dialect, String),

    #[error("The target '{0}' is misconfigured")]
    InvalidTargetConfiguration(String),

    #[error(transparent)]
    CAST(#[from] c::ast::ASTError),

    #[error(transparent)]
    CSema(#[from] c::sema_gen::SemaGenError<PatternError>),

    #[error("Failed to wait for task, {0}")]
    Concurrency(#[from] JoinError),
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
    pub async fn compile(
        &self, 
        tasks: &mut JoinSet<Result<(), TargetError>>, 
        graph: Arc<Mutex<ir::EntryGraph>>,
        config: Arc<Config>,
        diags: &impl DiagnosticReporter
    ) -> Result<usize, TargetError> {
        
        let base = config.base_path
            .to_str()
            .map_or(String::new(), |s| 
                s.to_string());

        let mut is_misconfigured = false;

        let pattern_results: Vec<(usize, glob::Paths)> = self.files.iter().enumerate()
            .filter_map(|(i, f)| glob((base.clone() + f).as_str())
                .map_err(|e| { 
                    is_misconfigured = true;
                    diags.diagnose(TargetError::InvalidGlobPattern(
                        self.name.clone(), f.clone(), e));
                    })
                .ok()
                .map(|paths| (i, paths))
            ).collect();

        if is_misconfigured {
            return Err(TargetError::InvalidTargetConfiguration(self.name.clone()));
        }

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

        Ok(0)
    }
}

pub async fn compile(
    targets: impl Iterator<Item = Target>,
    graph: Arc<Mutex<ir::EntryGraph>>,
    config: Arc<Config>,
    diags: &impl DiagnosticReporter
) -> Result<(), TargetError> {
    let mut tasks = JoinSet::<Result<(), TargetError>>::new();

    for target in targets {
        target.compile(&mut tasks, graph.clone(), config.clone(), diags).await?;
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(result) => result?,
            Err(join_error) => return Err(TargetError::Concurrency(join_error))
        };
    }

    Ok(())
}