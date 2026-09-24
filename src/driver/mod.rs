//! Unstable interface to the compiler.
//! Do not rely on this interface. It may change anytime.
//! Use the command-line interface instead.

use std::collections::HashMap;
use std::string::FromUtf16Error;
use std::{error, fs, str};
use std::path::{Path, PathBuf};
use std::str::{FromStr, Utf8Error};
use std::sync::{
    Arc, Mutex
};
use strict_yaml_rust::strict_yaml::Hash;
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
use crate::ir::entry_graph::{Attribute, Role};
use crate::{
    frontend,
    ir,
    compiler::{
        strings,
        diagnostics::{
            self,
            DiagnosticReporter
        }
    }
};
use thiserror;
pub mod manifest;

#[derive(Debug, Clone)]
pub struct Target {
    pub name: String,
    pub files: Vec<String>,
    pub language: Option<ir::Language>,
    pub language_config: frontend::LanguageSettings,
    pub dialect: ir::Dialect,
    pub dialect_config: frontend::DialectConfig,
    pub encoding: strings::Encoding
}

pub struct Product {
    pub format: String,
    pub targets: Option<Vec<String>>,
}

pub struct Config {
    pub base_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("The pattern '{1}' in target '{0}' is invalid: {2}")]
    InvalidGlobPattern(String, String, PatternError),

    #[error("The pattern '{1}' in target '{0}' is not resolvable: {2}")]
    Glob(String, String, GlobError),

    #[error("The {0} dialect specified in target '{1}' is not supported")]
    UnsupportedDialect(ir::Dialect, String),

    #[error("The {0} programming language specified in target '{1}' is not supported")]
    UnsupportedLanguage(ir::Language, String),

    #[error("The target '{0}' is misconfigured")]
    InvalidTargetConfiguration(String),

    #[error("Failed to wait for task, {0}")]
    Concurrency(#[from] JoinError),

    #[error("The file {0} used in target '{1}' could not be read: {2}")]
    UnreadableFile(PathBuf, String, std::io::Error),
    
    #[error(transparent)]
    CLanguageError(#[from] frontend::c::CLanguageError),
}

impl diagnostics::Diagnostic for TargetError {
    fn severity(&self) -> diagnostics::Severity {
        match self {
            _ => diagnostics::Severity::Error,
        }
    }
}

impl Target {
    pub async fn compile<R: DiagnosticReporter>(
        &self, 
        tasks: &mut JoinSet<Result<(), TargetError>>, 
        graph: Arc<Mutex<ir::EntryGraph>>,
        config: Arc<Config>,
        diags: Arc<R>
    ) -> Result<usize, TargetError> {
        
        let base = config.base_path
            .to_str()
            .map_or(String::new(), |s| 
                s.to_string());

        let mut is_misconfigured = false;

        let pattern_results: Vec<(usize, glob::Paths)> = self.files.iter()
            .enumerate()
            .filter_map(|(i, f)| {
                let path = PathBuf::from_str(f).expect("std::path returned Err, but is infallible; please file an bug report.");
                return glob(&(if path.is_absolute() {
                    String::new()
                } else {
                    base.clone()
                } + f))
                .map_err(|e| { 
                    is_misconfigured = true;
                    diags.diagnose(TargetError::InvalidGlobPattern(
                        self.name.clone(), f.clone(), e));
                    })
                .ok()
                .map(|paths| (i, paths))
            }).collect();

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

                debug!("Target {} has {}", self.name, path.display());

                count += 1;

                let target = self.clone();

                let graph = graph.clone();
                let config = config.clone();
                let diags = Arc::new(crate::compiler::diagnostics::ConsoleDiagnostics);

                tasks.spawn(async move {
                   target.compile_file(path, graph, &config, diags)
                });
            }
        }

        Ok(count)
    }

    pub fn compile_file<R: DiagnosticReporter>(
        &self,
        path: PathBuf,
        graph: Arc<Mutex<ir::EntryGraph>>,
        config: &Config,
        diags: Arc<R>
    ) -> Result<(), TargetError> {
        let data = fs::read(&path)
            .map_err(|e| TargetError::UnreadableFile(path.clone(), self.name.clone(), e))?;

        self.compile_unit(&data, path, graph, config, diags)
    }

    pub fn compile_unit<R: DiagnosticReporter>(
        &self,
        data: &[u8],
        path: PathBuf,
        graph: Arc<Mutex<ir::EntryGraph>>,
        config: &Config,
        diags: Arc<R>
    ) -> Result<(), TargetError> {
        match self.dialect {
            ir::Dialect::Doxygen => {
                let recorder = frontend::doxygen::DoxygenFrontend::new(
                    graph.clone(), 
                    &self.dialect_config.doxygen,
                    path,
                    self.language,
                    config,
                    diags.as_ref()
                );
                if self.language.is_some() {
                    self.compile_code(data, graph, config, diags.clone(), recorder)?;
                }
            },
            _ => return Err(TargetError::UnsupportedDialect(self.dialect, self.name.clone()))
        }

        Ok(())
    }

    fn compile_code<R: DiagnosticReporter, CR: frontend::SymbolCommentIngestingFrontend>(
        &self,
        data: &[u8],
        graph: Arc<Mutex<ir::EntryGraph>>,
        config: &Config,
        diags: Arc<R>,
        recorder: CR
    ) -> Result<(), TargetError> {
        if let Some(language) = self.language {
            match language {
                ir::Language::C => {
                    frontend::c::gen_sema(self.encoding, 
                        data, 
                        &self.language_config.c, 
                        frontend::c::sema_gen::CFrontend(recorder), 
                        diags.as_ref())?;
                }

                language => {
                    return Err(TargetError::UnsupportedLanguage(language, self.name.clone()))
                }
            }
        }

        Ok(())
    }
}

pub async fn compile(
    targets: impl Iterator<Item = Target>,
    graph: Arc<Mutex<ir::EntryGraph>>,
    config: Arc<Config>,
    diags: Arc<impl DiagnosticReporter>
) -> Result<(), TargetError> {
    let mut tasks = JoinSet::<Result<(), TargetError>>::new();

    for target in targets {
        // Compile target. Compilation of the targeted files will be performed in parallel
        // using the task group from above.
        target.compile(&mut tasks, graph.clone(), config.clone(), diags.clone()).await?;
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(result) => result?,
            Err(join_error) => return Err(TargetError::Concurrency(join_error))
        };
    }

    Ok(())
}