use enum_assoc::Assoc;
use figment::{Figment, Provider};
use log::{
    debug
};
use clap::{
    Parser
};
use strum_macros::{EnumDiscriminants, EnumString};
use thiserror;
use simple_logger::SimpleLogger;
use std::{
    borrow::Cow, fmt::{Display, Write}, fs, io::{self, IsTerminal, Read}, path::{
        Path, PathBuf
    }, process::ExitCode, str::FromStr, sync::{Arc, Mutex}, vec,
};
use json5;
use encoding::{self, Encoding};
use argon::{
    backend, compiler::{
        diagnostics::{
            self, DiagnosticReporter, EditorLocationExtension, FileAttachableDiagnostic, FileDiagnosticsExtension, MinimalLocation
        }, strings
    }, driver::{self, manifest::v1::DocManifest}, frontend, ir
};

#[derive(Debug, thiserror::Error)]
enum CLIError {
    #[error("No config file path specified and neither of doc.json or [doc|docs|Documentation]/doc.[json|yaml] exist")]
    MissingConfigFile,

    #[error("Manifest could not be transcoded from UTF-16 to UTF-8, {0}.")]
    ManifestTranscodingFailure(Cow<'static, str>),

    #[error("Manifest does not represent valid UTF-8, {0}.")]
    ManifestEncodingError(#[from] std::str::Utf8Error),

    #[error("Manifest could not be parsed, {0}")]
    JSONParsingFailed(#[from] json5::Error),

    #[error("Manifest could not be parsed, {0}")]
    StrictYAMLParsingFailed(#[from] strict_yaml_rust::serde::error::Error),

    #[error("Specified manifest version {0} is not supported by this compiler.")]
    UnsupportedManifestVersion(semver::Version),

    #[error("Specified manifest format '{0}' is not supported by this compiler.")]
    UnsupportedManifestFormat(ManifestFormat),

    #[error("Manifest at {0} is not readable, {1}")]
    UnreadableManifest(PathBuf, io::Error),
}

impl diagnostics::Diagnostic<MinimalLocation> for CLIError {
    fn location(&self) -> Option<MinimalLocation> {
        match self {
            Self::JSONParsingFailed(e) => e.location().map(|l| l.to_minimal_location()),
            Self::StrictYAMLParsingFailed(e) => e.location().map(|l| l.to_minimal_location()),
            _ => None
        }
    }
}

#[derive(Parser, Debug, Clone, Assoc)]
#[func(pub const fn keyword(&self) -> &'static str)]
enum ManifestFormat {
    #[assoc(keyword = "json")]
    JSON,

    #[assoc(keyword = "yaml")]
    StrictYAML,

    #[assoc(keyword = "toml")]
    TOML,
}

impl Display for ManifestFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.keyword())
    }
}

impl AsRef<str> for ManifestFormat {
    fn as_ref(&self) -> &str {
        self.keyword()
    }
}


impl FromStr for ManifestFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "json" | "JSON" => Ok(Self::JSON),
            "yml" | "yaml" | "YAML" => Ok(Self::StrictYAML),
            "toml" | "TOML" => Ok(Self::TOML),
            s => Err(format!("Unknown and unsupported manifest format '{s}'"))
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

    #[arg(long, default_value_t = None)]
    manifest_format: Option<ManifestFormat>
}

const ENV_PREFIX: &'static str = "DOC_";

fn try_well_known_config_paths() -> Option<PathBuf> {
    debug!("No config file specified, looking at known locations");
    let paths: [&Path; 8] = [
        "doc.json", 
        "doc/doc.json", 
        "docs/doc.json", 
        "Documentation/doc.json",
        "doc.toml", 
        "doc/doc.toml", 
        "docs/doc.toml", 
        "Documentation/doc.toml",
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

    debug!("Using manifest at {}", config_file.display());

    fs::read(config_file.as_path())
        .map_err(|e| CLIError::UnreadableManifest(config_file.clone(), e))
        .map(|data| (data, config_file, false))
}

impl Provider for DocManifest {

}

fn parse_manifest(data: &[u8], format: ManifestFormat, args: &Arguments, diags: &impl DiagnosticReporter) -> Result<argon::driver::manifest::v1::DocManifest, CLIError> {
    debug!("Using manifest formatted as {format}");
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

    let (manifest, preparsed) = match format {
        ManifestFormat::JSON => {
            let preparsed_manifest: argon::driver::manifest::PreparsedManifest = json5::from_str(str)?;

            if preparsed_manifest.version.major != 1 {
                return Err(CLIError::UnsupportedManifestVersion(preparsed_manifest.version))
            }

            let manifest: argon::driver::manifest::v1::DocManifest = json5::from_str(str)?;

            Ok((manifest, preparsed_manifest))
        }
        ManifestFormat::TOML => {
            // wtf, no.
            Err(CLIError::UnsupportedManifestFormat(ManifestFormat::TOML))
        }
        ManifestFormat::StrictYAML => {
            // also wtf, but acceptable bc strict yaml.
            let preparsed_manifest: argon::driver::manifest::PreparsedManifest = strict_yaml_rust::serde::from_str(str)?;

            if preparsed_manifest.version.major != 1 {
                return Err(CLIError::UnsupportedManifestVersion(preparsed_manifest.version))
            }

            let manifest: argon::driver::manifest::v1::DocManifest = strict_yaml_rust::serde::from_str(str)?;

            Ok((manifest, preparsed_manifest))
        }
    }?;

    if preparsed.version.major == 1 {
        let env_overrides = match envy::prefixed(ENV_PREFIX).from_env::<DocManifest>() {
            Ok(manifest) => Some(manifest),
            Err(err) => None,
        };

        let overriden_manifest: argon::driver::manifest::v1::DocManifest = Figment::new()
            .merge(manifest)
            .merge(figment::providers::Env::prefixed(ENV_PREFIX))
            .extract().or(manifest)?;

        Ok(overriden_manifest)
    } else {
        Ok(manifest)
    }
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

    let format = manifest_path
        .extension()
        .map(|s| ManifestFormat::from_str(s.to_str())?)
        .flatten()
        .unwrap_or(args.manifest_format);

    let manifest = match parse_manifest(&manifest, format, &args, &diags) {
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

    let cwd = std::env::current_dir().ok();

    if cwd.is_none() {
        debug!("could not retrieve current working dir");
    }

    let base_path = if from_stdin {
        cwd.unwrap_or(PathBuf::new())
    } else {
        manifest_path.parent().map_or(PathBuf::new(), |p| p.to_path_buf())
    };

    let graph = Arc::new(Mutex::new(ir::EntryGraph::new(
        manifest.roles(), 
        manifest.attributes()
    )));

    let config = Arc::new(driver::Config {
        base_path,
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
