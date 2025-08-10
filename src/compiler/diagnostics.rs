use std::fmt::{Debug, Display};
use std::marker::PhantomData;
use std::{error, path::PathBuf};
use std::ops::Range;
use colored::{control::SHOULD_COLORIZE, Color, Colorize};
pub use super::strings::{
    Location,
    Point,
    Source
};

pub trait EditorLocation {
    fn start_point(&self) -> Point; // row, column
    fn start_offset(&self) -> Option<usize> { None }
    fn end_offset(&self) -> Option<usize> { None }
    fn end_point(&self) -> Option<Point> { None }
}

pub trait Diagnostic<L: EditorLocation = Location>: error::Error {
    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn is_internal(&self) -> bool {
        false
    }

    fn location(&self) -> Option<L> {
        None
    }

    fn file(&self) -> Option<PathBuf> {
        None
    }
}

pub struct LocatedDiagnostic<D: Diagnostic<OgL>, NewL: EditorLocation, OgL: EditorLocation>(D, NewL, PhantomData<OgL>);

impl<D: Diagnostic<OgL>, OgL: EditorLocation, NewL: EditorLocation> Debug for LocatedDiagnostic<D, NewL, OgL> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<D: Diagnostic<OgL>, OgL: EditorLocation, NewL: EditorLocation> Display for LocatedDiagnostic<D, NewL, OgL> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<D: Diagnostic<OgL>, OgL: EditorLocation, NewL: EditorLocation> error::Error for LocatedDiagnostic<D, NewL, OgL> {}

impl<D: Diagnostic<OgL>, OgL: EditorLocation, NewL: EditorLocation + Clone> Diagnostic<NewL> for LocatedDiagnostic<D, NewL, OgL> {
    fn is_internal(&self) -> bool {
        self.0.is_internal()
    }

    fn severity(&self) -> Severity {
        self.0.severity()
    }

    fn location(&self) -> Option<NewL> {
        Some(self.1.clone())
    }
}
pub trait LocationAttachableDiagnostic<OgL: EditorLocation>: Diagnostic<OgL> + Sized {
    fn at<NewL: EditorLocation>(self, location: NewL) -> LocatedDiagnostic<Self, NewL, OgL> {
        LocatedDiagnostic(self, location, PhantomData)
    }
}

impl<T, OgL: EditorLocation> LocationAttachableDiagnostic<OgL> for T where T: Diagnostic<OgL> + Sized {}

pub struct FileDiagnostic<D: Diagnostic<L>, L: EditorLocation>(D, PathBuf, PhantomData<L>);

impl<D: Diagnostic<L>, L: EditorLocation> Debug for FileDiagnostic<D, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<D: Diagnostic<L>, L: EditorLocation> Display for FileDiagnostic<D, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<D: Diagnostic<L>, L: EditorLocation> error::Error for FileDiagnostic<D, L> {}

impl<D: Diagnostic<L>, L: EditorLocation> Diagnostic<L> for FileDiagnostic<D, L> {
    fn is_internal(&self) -> bool {
        self.0.is_internal()
    }

    fn severity(&self) -> Severity {
        self.0.severity()
    }

    fn location(&self) -> Option<L> {
        self.0.location()
    }

    fn file(&self) -> Option<PathBuf> {
        Some(self.1.clone())
    }
}

pub trait FileAttachableDiagnostic<L: EditorLocation>: Diagnostic<L> + Sized {
    fn inside(self, file: PathBuf) -> FileDiagnostic<Self, L> {
        FileDiagnostic(self, file, PhantomData)
    }
}

impl<T, L: EditorLocation> FileAttachableDiagnostic<L> for T where T: Diagnostic<L> + Sized {}

impl EditorLocation for Location {
    fn start_point(&self) -> Point {
        self.start_point
    }

    fn end_point(&self) -> Option<Point> {
        Some(self.end_point)
    }

    fn start_offset(&self) -> Option<usize> {
        Some(self.start_byte)
    }

    fn end_offset(&self) -> Option<usize> {
        Some(self.end_byte)
    }
}

impl EditorLocation for json5::Location {
    fn start_point(&self) -> Point {
        Point { row: self.line, column: self.column }
    }
}

impl Diagnostic<json5::Location> for json5::Error {
    fn location(&self) -> Option<json5::Location> {
        match self {
            Self::Message { location, .. } => location.clone()
        }
    }
}

pub trait DiagnosticReporter {
    fn diagnose<D: Diagnostic<L>, L: EditorLocation>(&self, diagnostic: D);
}

#[derive(Clone)]
pub struct ConsoleDiagnostics;

impl DiagnosticReporter for ConsoleDiagnostics {
    fn diagnose<D: Diagnostic<L>, L: EditorLocation>(&self, diagnostic: D) {
        if diagnostic.severity() >= Severity::Warning {
            if SHOULD_COLORIZE.should_colorize() {

                let prefix = if diagnostic.is_internal() {
                   (diagnostic.severity().as_str().to_string() + "internal").color(diagnostic.severity().internal_color()).bold()
                } else {
                    diagnostic.severity().as_str().color(diagnostic.severity().color()).bold()
                };
                

                if let Some(file) = diagnostic.file() {
                    if let Some(location) = diagnostic.location() {
                        eprintln!(
                            "{}:{}:{}: {}: {}",
                            file.display(),
                            location.start_point().row,
                            location.start_point().column,
                            prefix,
                            diagnostic
                        )
                    } else {
                        eprintln!(
                            "{}: {}: {}",
                            file.display(),
                            prefix,
                            diagnostic
                        )
                    }
                } else {
                    eprintln!(
                        "{}: {}",
                        prefix,
                        diagnostic
                    )
                }
            }
        }
    }
}

pub struct FileDiagnostics<R: DiagnosticReporter + Sized>(R, PathBuf);

impl<R: DiagnosticReporter> DiagnosticReporter for FileDiagnostics<R> {
    fn diagnose<D: Diagnostic<L>, L: EditorLocation>(&self, diagnostic: D) {
        self.0.diagnose(diagnostic.inside(self.1.clone()));
    }
}

pub trait FileDiagnosticsExtension: DiagnosticReporter + Sized {
    fn inside(self, file: PathBuf) -> FileDiagnostics<Self> {
        FileDiagnostics(self, file)
    }
}

impl<R: DiagnosticReporter + Sized> FileDiagnosticsExtension for R {}

macro_rules! declare_enum_case_macro {
    ($name: ident, $($property_name: ident, $property_type: ty),+) => {
        declare_enum_case_macro!{($) $name, $($property_name, $property_type)+}
    };

    (($D:tt) $name: ident, $($property_name: ident, $property_type: ty)+) => {
            macro_rules! $name {
            (
                $D(#[$enum_attrs:meta])*
                $visibility: vis enum $enum_name: ident <$D($g:ident : $generics:ty),*> {
                    $D(
                        #[$($property_name = $D$property_name: expr),+]
                        $D(#[$variant_attrs: meta])*
                        $variant: ident
                    ),*
                    $D(,)?
                },
            ) => {
                $D(#[$enum_attrs])*
                $visibility enum $enum_name <$D($generics:tt),*> {
                    $D(
                        $D(#[$variant_attrs])*
                        $variant,
                    )*
                }
                $(
                impl $enum_name {
                    fn $property_name(&self) -> $property_type {
                        match self {
                            $D(Self::$variant => $D$property_name,)*
                        }
                    }
                }
                )*
            }
        }
        
    }
}

#[derive(PartialEq, PartialOrd, Eq, Clone, Copy)]
#[repr(i8)]
pub enum Severity {
    Error = 2,
    Warning = 1,
    Info = 0,
    Debug = -1
}

impl Severity {
    fn level(&self) -> u8 {
        *self as u8
    }

    fn color(&self) -> Color {
        match self {
            Self::Error => Color::BrightRed,
            Self::Warning => Color::BrightYellow,
            Self::Info => Color::BrightBlue,
            Self::Debug => Color::BrightWhite,
        }
    }

    fn internal_color(&self) -> Color {
        match self {
            Self::Error => Color::BrightMagenta,
            Self::Warning => Color::Yellow,
            Self::Info => Color::Blue,
            Self::Debug => Color::BrightWhite,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

impl Into<&'static str> for Severity {
    fn into(self) -> &'static str {
        self.as_str()
    }
}

impl Into<Color> for Severity {
    fn into(self) -> Color {
        self.color()
    }
}

impl Ord for Severity {
    fn cmp(&self, other: &Severity) -> std::cmp::Ordering {
        self.level().cmp(&other.level())
    }
}