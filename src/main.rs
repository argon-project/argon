use log::{
    debug
};
use clap::{
    Parser
};
use thiserror;
use simple_logger::SimpleLogger;
use std::{
    borrow::Cow, 
     fs, 
     io::{self, IsTerminal, Read}, 
     path::{
        Path, PathBuf
    }, 
    process::ExitCode, 
    str::FromStr, 
    sync::{Arc, Mutex}, 
    vec,
};
use json5;
use encoding::{self, Encoding};
use argon::{
    compiler::{
        diagnostics::{
            self, DiagnosticReporter, FileAttachableDiagnostic, FileDiagnosticsExtension
        },
        strings
    }, 
    driver, 
    ir,
    frontend,
    backend,
};

#[derive(Debug, thiserror::Error)]
enum CLIError {
    #[error("No config file path specified and neither of doc.json or [doc|docs|Documentation]/doc.json exist")]
    MissingConfigFile,

    #[error("Manifest could not be transcoded from UTF-16 to UTF-8, {0}. The UTF-8 mandate is a limitation of the json5 parser.")]
    ManifestTranscodingFailure(Cow<'static, str>),

    #[error("Manifest does not represent valid UTF-8, {0}.")]
    ManifestEncodingError(#[from] std::str::Utf8Error),

    #[error("Manifest could not be parsed, {0}")]
    JSONParsingFailed(#[from] json5::Error),

    #[error("Specified manifest version {0} is not supported by this compiler.")]
    UnsupportedManifestVersion(semver::Version),

    #[error("Manifest at {0} is not readable, {1}")]
    UnreadableManifest(PathBuf, io::Error),
}

impl diagnostics::Diagnostic<json5::Location> for CLIError {
    fn location(&self) -> Option<json5::Location> {
        match self {
            Self::JSONParsingFailed(e) => e.location(),
            _ => None
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    #[arg(long, default_value_t = true)]
    debug: bool,

    #[arg(long)]
    manifest: Option<PathBuf>,

    #[arg(long, default_value_t = strings::Encoding::UTF8)]
    manifest_encoding: strings::Encoding,
}

fn try_well_known_config_paths() -> Option<PathBuf> {
    debug!("No config file specified, looking at known locations");
    let paths: [&Path; 4] = [
        "doc.json", 
        "doc/doc.json", 
        "docs/doc.json", 
        "Documentation/doc.json",
    ].map(|p| Path::new(p));

    paths.iter()
        .find(|p| p.exists())
        .map(|p| p.to_path_buf())
}

fn read_manifest(file: Option<PathBuf>) -> Result<(Vec<u8>, PathBuf, bool), CLIError> {
    let mut stdin = io::stdin();
    let is_forced_stdin = file.as_ref().map(|f| f.as_os_str() == "<stdin>") == Some(true);
    if is_forced_stdin {
        let mut buffer: Vec<u8> = vec![];
        stdin.read_to_end(&mut buffer)
            .map_err(|e| CLIError::UnreadableManifest(PathBuf::from_str("<stdin>").unwrap(), e))?;
        return Ok((buffer, PathBuf::from_str("<stdin>").unwrap(), true))
    }
    else if file.is_none() && !stdin.is_terminal() {
        let mut buffer: Vec<u8> = vec![];
        if stdin.read_to_end(&mut buffer).is_ok()  {
            if !buffer.is_empty() {
                debug!("Using stdin as config file");
                return Ok((buffer, PathBuf::from_str("<stdin>").unwrap(), true))
            }
        }
    }

    let config_file = file
        .or_else(try_well_known_config_paths)
        .ok_or(CLIError::MissingConfigFile)?;

    debug!("Using config file at {}", config_file.display());

    fs::read(config_file.as_path())
        .map_err(|e| CLIError::UnreadableManifest(config_file.clone(), e))
        .map(|data| (data, config_file, false))
}

fn parse_manifest(data: &[u8], args: &Arguments) -> Result<argon::driver::manifest::v1::DocManifest, CLIError> {
    let transcoded: Option<String> = match args.manifest_encoding {
        strings::Encoding::UTF8 => None,
        strings::Encoding::UTF16(strings::Endianness::LittleEndian) => {
            Some(encoding::all::UTF_16LE.decode(&data, encoding::DecoderTrap::Strict)
                .map_err(|e| CLIError::ManifestTranscodingFailure(e))?)
        },
        strings::Encoding::UTF16(strings::Endianness::BigEndian) => {
            Some(encoding::all::UTF_16BE.decode(&data, encoding::DecoderTrap::Strict)
                .map_err(|e| CLIError::ManifestTranscodingFailure(e))?)
        },
    };

    let str: &str = match args.manifest_encoding {
        strings::Encoding::UTF8 => str::from_utf8(&data)?,
        strings::Encoding::UTF16(_) => &transcoded.expect("Bug!"),
    };

    let preparsed_manifest: argon::driver::manifest::PreparsedManifest = json5::from_str(str)?;

    if preparsed_manifest.version.major != 1 {
        return Err(CLIError::UnsupportedManifestVersion(preparsed_manifest.version))
    }

    let manifest: argon::driver::manifest::v1::DocManifest = json5::from_str(str)?;

    Ok(manifest)
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Arguments::parse();

    if args.debug {
        SimpleLogger::new().init().unwrap();
    }

    let diags = diagnostics::ConsoleDiagnostics;

    let (manifest, manifest_path, from_stdin) = match read_manifest(args.manifest.clone()) {
        Ok(x) => x,
        Err(e) => {
            diags.diagnose(e);
            return ExitCode::FAILURE
        },
    };

    let manifest = match parse_manifest(&manifest, &args) {
            Ok(m) => m,
            Err(e) => {
                diags.diagnose(e.inside(manifest_path));
                return ExitCode::FAILURE
            },
        };

    if !manifest.proofread(&diags.clone().inside(manifest_path.clone())) {
        debug!("Manifest is invalid");
        return ExitCode::FAILURE;
    }

    debug!("{:?}", manifest);

    let graph = Arc::new(Mutex::new(ir::EntryGraph::new()));

    let cwd = std::env::current_dir().ok();

    if cwd.is_none() {
        debug!("could not retrieve current working dir");
    }

    let base_path = if from_stdin {
        cwd.unwrap_or(PathBuf::new())
    } else {
        manifest_path.parent().map_or(PathBuf::new(), |p| p.to_path_buf())
    };

    let config = Arc::new(driver::Config {
        base_path,
        roles: manifest.roles.into(),
        attributes: manifest.attributes
            .iter()
            .map(|a| (a.id.clone(), a.into()))
            .collect(),
        doxygen: driver::DoxygenSettings { 
            commands: frontend::doxygen::commands::builtins()
        },
        c: driver::CLanguageSettings { 
            sema_gen: frontend::c::sema_gen::Config::default()
        }
    });
    
    let diags = Arc::new(diags);
    
    match driver::compile(manifest.targets(), graph, config, diags.clone()).await {
        Ok(()) => {
            debug!("Sucessfully compiled targets")
        },
        Err(e) => {
            diags.diagnose(e);
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}
