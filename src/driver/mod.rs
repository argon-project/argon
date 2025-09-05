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
    compiler::{
        strings,
        diagnostics::{
            self,
            DiagnosticReporter
        }
    }
};
use thiserror;

use crate::frontend::{c, DocumentationCommentRecorder};
use crate::ir;

pub mod manifest;

#[derive(Debug, Clone)]
pub struct Target {
    pub name: String,
    pub files: Vec<String>,
    pub language: Option<ir::Language>,
    pub dialect: ir::Dialect,
    pub encoding: strings::Encoding
}

pub struct Product {
    pub format: String,
    pub targets: Option<Vec<String>>,
}

pub struct CLanguageSettings {
    pub sema_gen: c::sema_gen::Config,
}

pub struct DoxygenSettings {
    pub commands: HashMap<String, frontend::doxygen::DoxygenCommand<'static>>,
}

pub struct Config {
    pub base_path: PathBuf,
    pub doxygen: DoxygenSettings,
    pub c: CLanguageSettings,

    pub roles: Role,
    pub attributes: HashMap<String, Attribute>,
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

    #[error(transparent)]
    CAST(#[from] c::ast::ASTError),

    #[error(transparent)]
    CSemaUTF8(#[from] c::sema_gen::SemaGenError<Utf8Error>),

     #[error(transparent)]
    CSemaUTF16(#[from] c::sema_gen::SemaGenError<FromUtf16Error>),

    #[error("Failed to wait for task, {0}")]
    Concurrency(#[from] JoinError),

    #[error("The file {0} used in target '{1}' could not be read: {2}")]
    UnreadableFile(PathBuf, String, std::io::Error),
}

impl diagnostics::Diagnostic for TargetError {
    fn severity(&self) -> diagnostics::Severity {
        match self {
            Self::CAST(e) => e.severity(),
            Self::CSemaUTF8(e) => e.severity(),
            Self::CSemaUTF16(e) => e.severity(),
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
                let path = PathBuf::from_str(f).expect("std::path returned Err, but is infallible");
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

    fn compile_file<R: DiagnosticReporter>(
        &self,
        path: PathBuf,
        graph: Arc<Mutex<ir::EntryGraph>>,
        config: &Config,
        diags: Arc<R>
    ) -> Result<(), TargetError> {
        let data = fs::read(&path)
            .map_err(|e| TargetError::UnreadableFile(path.clone(), self.name.clone(), e))?;

        match self.dialect {
            ir::Dialect::Doxygen => {
                let mut recorder = frontend::doxygen::DoxygenRecorder::new(
                    graph.clone(), 
                    &config.doxygen
                );
                if self.language.is_some() {
                    self.compile_source_code(data, graph, config, diags, &recorder)?;
                }
            },
            _ => return Err(TargetError::UnsupportedDialect(self.dialect, self.name.clone()))
        }

        Ok(())
    }

    fn compile_source_code<R: DiagnosticReporter, CR: DocumentationCommentRecorder>(
        &self,
        data: Vec<u8>,
        graph: Arc<Mutex<ir::EntryGraph>>,
        config: &Config,
        diags: Arc<R>,
        recorder: &CR
    ) -> Result<(), TargetError> {
        if let Some(language) = self.language {
            match language {
                ir::Language::C => {
                    let mut recorder = frontend::c::sema_gen::CRecorder::new(recorder);
                    match self.encoding {
                        strings::Encoding::UTF8 => {
                        let ast = frontend::c::ast::gen_ast_utf8(data.as_slice(), None)?;
                           frontend::c::sema_gen::gen_sema(
                                ast.root_node(), 
                                data.as_slice(), 
                                &config.c.sema_gen, 
                                &mut recorder, 
                                diags.as_ref()
                            )
                            .map_err(|e| TargetError::CSemaUTF8(e))?;
                        }
                        strings::Encoding::UTF16(endianness) => {
                            let (_, middle, _) = unsafe { data.align_to::<u16>() };
                            let ast = frontend::c::ast::gen_ast_utf16(middle, None, endianness)?;
                            frontend::c::sema_gen::gen_sema(
                                ast.root_node(), 
                                middle, 
                                &config.c.sema_gen, 
                                &mut recorder, 
                                diags.as_ref()
                            )
                            .map_err(|e| TargetError::CSemaUTF16(e))?;
                        }
                    };
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