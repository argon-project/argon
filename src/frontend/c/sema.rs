#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use std::fmt;

use strum_macros::EnumString;

use crate::ir::SymbolLikeness;

#[derive(Clone)]
pub enum Symbol {
    function(Function),
    variable(Variable),
    typeDefinition(TypeDefinition),
    structure(Struct),
    union(Union),
    enumeration(Enum),
    macroDefintion(MacroDefinition),
    // header(TranslationUnit),
    // source(TranslationUnit),
}

impl Symbol {
    pub fn likeness(&self) -> SymbolLikeness {
        match self {
            Self::function(_) => SymbolLikeness::Function,
            Self::variable(_) => SymbolLikeness::Variable,
            Self::typeDefinition(definition) => definition.originalType.decl.likeness(),
            Self::structure(_) | Self::union(_) => SymbolLikeness::Structure,
            Self::enumeration(_) => SymbolLikeness::Enumeration,
            Self::macroDefintion(definition) => if definition.parameters.is_empty() {
                SymbolLikeness::Constant
            } else {
                SymbolLikeness::Function
            },
            // Self::header(_) | Self::source(_) => SymbolLikeness::File,
        }
    }
}

impl TypeDecl {
    pub fn likeness(&self) -> SymbolLikeness {
        match self {
            Self::function(_) => SymbolLikeness::Function,
            Self::enumeration(_) => SymbolLikeness::Enumeration,
            Self::structure(_) | Self::union(_) => SymbolLikeness::Structure,
            Self::pointer(pointer) => pointer.pointeeType.decl.likeness(),
            _ => SymbolLikeness::Statement,
        }
    }
}

// TODO: include graph? -> would require us to include paths

#[derive(Clone)]
pub struct TranslationUnit {
    pub includes: Vec<IncludedHeader>,
}

#[derive(Clone)]
pub enum IncludedHeader {
    system(String),
    local(String),
}

pub type Statements = String;
pub type Expression = String;
pub type RawSpelling = String;
pub type Identifier = String;

#[derive(Clone)]
pub struct PreprocessorCall {
    /// Name
    directive: RawSpelling,

    arguments: Vec<RawSpelling>,
}

#[derive(Clone)]
pub struct MacroDefinition {
    pub name: Identifier,

    pub parameters: Vec<Parameter<Identifier>>,

    pub value: Option<RawSpelling>
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum Storage {
    #[strum(serialize = "extern")]
    r#extern,

    #[strum(serialize = "static")]
    r#static,

    #[strum(serialize = "register")]
    register,

    #[strum(
        to_string = "inline",
        serialize = "__forceinline",
        serialize = "__inline__",
        serialize = "__inline",
        serialize = "inline",
    )]
    inline,

    #[strum(serialize = "thread_local", serialize = "__thread", to_string = "thread_local")]
    threadLocal,
}

/// Declaration modifier
#[derive(Clone)]
pub enum Modifier {
    /// Storage class modifier
    storage(Storage),

    /// Old-school, non-standard `__attribute__` or `__attribute`
    attribute(Attribute),

    /// C++20 `[[prefix:attribute]]` attribute syntax
    standardAttribute(StandardAttribute),

    /// Microsoft-specifc modifiers
    msSpecificModifier(MSDeclModifier),
}

#[derive(Clone)]
pub enum MSDeclModifier {
    /// `__declspec` modifier
    declspec(Identifier),
}

#[derive(Clone)]
pub enum DeclarationQualifier {
    /// Constant expression, applies to variables and functions
    constexpr,

    /// Suppresses warnings, applies to variable/function
    extension,

    /// Align as, applies to variable
    alignas(Expression),
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum PointerQualifier {
    /// Not null, applies to pointers only
    #[strum(serialize = "_Nonnull")]
    nonnull,

    /// May be null, applies to pointers only
    #[strum(serialize = "_Nullable")]
    nullable,

    /// Only reference to the pointee for the duration of access to this pointer
    #[strum(serialize = "restrict")]
    restrict,
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum FunctionQualifier {
    /// Function does not return, applies to functions only (not function pointers)
    #[strum(serialize = "_Noreturn")]
    noreturn,
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum ArrayQualifier {
    /// Passed array pointer has at least `N` elements,
    /// when count is given in the form `type_t array[static N];`
    #[strum(serialize = "static")]
    r#static,

    /// Only reference to the pointee for the duration of access to this pointer
    /// Also used for array parameters, as these decay to pointers
    #[strum(serialize = "restrict")]
    restrict,
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum TypeQualifier {
    /// Immutable value, applies to types
    #[strum(serialize = "const")]
    r#const,

    /// Value might change unexpectedly, don't optimize access to it, applies to types
    #[strum(serialize = "volatile")]
    volatile,

    /// Atomic value, applies to types
    #[strum(serialize = "_Atomic")]
    atomic,
}

#[derive(Clone)]
pub struct Attribute {
    pub arguments: Vec<RawSpelling>, // TODO: more detail?
}

#[derive(Clone)]
pub struct StandardAttribute {
    pub prefix: Option<Identifier>,
    pub name: Identifier,
    pub arguments: Vec<RawSpelling>, // TODO: more detail?
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum AssemblyFlavor {
    #[strum(serialize = "volatile")]
    volatile,

    #[strum(serialize = "inline")]
    inline,

    #[strum(serialize = "goto")]
    goto,
}

/// `asm(), __asm(), or __asm__()`
#[derive(Clone)]
pub struct InlineAssembly {
    flavors: Vec<AssemblyFlavor>,

    // TODO: more details?
    arguments: Expression,
}

// MARK: - C Types

#[derive(Clone)]
pub enum TypeDecl {
    structure(Struct),
    union(Union),
    enumeration(Enum),

    // FIXME: function pointer special
    pointer(Pointer),
    function(Function),
    array(Array),
    macroCall(Expression),

    named(NamedType, /* sized: */ Vec<SizeModifier>),
}

#[derive(Clone)]
pub enum NamedType {
    custom(Identifier),
    primitive(PrimitiveType),
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum SizeModifier {
    #[strum(serialize = "signed")]
    signed,

    #[strum(serialize = "unsigned")]
    unsigned,

    #[strum(serialize = "long")]
    long,

    #[strum(serialize = "short")]
    short,
}

#[derive(Clone)]
pub struct CType {
    pub decl: Box<TypeDecl>,
    pub qualifiers: Vec<TypeQualifier>,
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum PrimitiveType {
    #[strum(serialize = "bool")]
    bool,

    #[strum(serialize = "char")]
    char,

    #[strum(serialize = "int")]
    int,

    #[strum(serialize = "float")]
    float,

    #[strum(serialize = "double")]
    double,

    #[strum(serialize = "void")]
    void,

    #[strum(serialize = "size_t")]
    size,

    #[strum(serialize = "ssize_t")]
    ssize,

    #[strum(serialize = "ptrdiff_t")]
    ptrdiff,

    #[strum(serialize = "intptr_t")]
    intptr,

    #[strum(serialize = "uintptr_t")]
    uintptr,

    #[strum(serialize = "charptr_t")]
    charptr,

    #[strum(serialize = "nullptr_t")]
    nullptr,

    #[strum(serialize = "max_align_t")]
    maxAlign,

    #[strum(serialize = "int8_t")]
    int8,

    #[strum(serialize = "int16_t")]
    int16,

    #[strum(serialize = "int32_t")]
    int32,

    #[strum(serialize = "int64_t")]
    int64,

    #[strum(serialize = "uint8_t")]
    uint8,

    #[strum(serialize = "uint16_t")]
    uint16,

    #[strum(serialize = "uint32_t")]
    uint32,

    #[strum(serialize = "uint64_t")]
    uint64,

    #[strum(serialize = "char8_t")]
    char8,

    #[strum(serialize = "char16_t")]
    char16,

    #[strum(serialize = "char32_t")]
    char32,

    #[strum(serialize = "char64_t")]
    char64,
}

#[derive(Clone)]
pub struct Array {
    pub count: Option<Expression>,

    pub elementType: CType,

    pub qualifiers: Vec<ArrayQualifier>,
}

#[derive(Clone)]
pub enum MSPointerModifier {
    unaligned,
    restrict,
    unsigned,
    signed,
    /// `__based`
    based(/* parameters: */ Vec<RawSpelling>),
}

#[derive(Clone)]
pub struct Pointer {
    pub msSpecificModifiers: Vec<MSPointerModifier>,

    pub qualifiers: Vec<PointerQualifier>,

    pub pointeeType: CType,
}

#[derive(Clone)]
pub struct TypeDefinition {
    pub qualifiers: Vec<DeclarationQualifier>,

    pub name: Identifier,

    pub originalType: CType,

    pub attributes: Vec<Attribute>,
}

#[derive(Clone)]
pub enum AttachedExpression {
    bitWidth(Expression),
    value(Expression),
}

#[derive(Clone)]
pub struct Variable {
    // declaration specifiers: --------
    pub modifiers: Vec<Modifier>,

    pub qualifiers: Vec<DeclarationQualifier>,

    pub cType: CType, // form from specifiers AND each field declarator (which may be a function),

    pub identifier: Option<Identifier>,

    // This is to enable using Variable both for struct/union fields and variables with initializers.
    pub attachedExpression: Option<AttachedExpression>,
}

#[derive(Clone)]
pub struct Container {
    pub name: Option<Identifier>,
    pub attributes: Vec<Attribute>,
    pub msSpecificModifiers: Vec<MSDeclModifier>,
    pub members: Vec<Variable>,
}

pub type Struct = Container;
pub type Union = Container;

#[derive(Clone)]
pub struct EnumCase {
    pub name: Identifier,
    pub value: Option<Expression>,
}

#[derive(Clone)]
pub struct Enum {
    pub name: Option<Identifier>,
    pub attributes: Vec<Attribute>,

    pub underlyingType: Option<PrimitiveType>,

    pub cases: Vec<EnumCase>,
}

// MARK: - Functions

#[derive(Clone)]
pub enum Parameter<R> {
    variadic(Option<Identifier>),
    regular(R),
}

#[derive(Clone, EnumString, strum_macros::Display, strum_macros::AsRefStr)]
pub enum MSCallModifier {
    #[strum(serialize = "__cdecl")]
    cdecl,

    #[strum(serialize = "__clrcall")]
    clrcall,

    #[strum(serialize = "__stdcall")]
    stdcall,

    #[strum(serialize = "__fastcall")]
    fastcall,

    #[strum(serialize = "__thiscall")]
    thiscall,

    #[strum(serialize = "__vectorcall")]
    vectorcall,
}

#[derive(Clone)]
pub struct Function {
    /// Declaration modifiers
    pub modifiers: Vec<Modifier>,

    pub functionQualifiers: Vec<FunctionQualifier>,

    pub declQualifiers: Vec<DeclarationQualifier>,

    /// Type specifier
    pub returnType: CType,

    pub msSpecifcModifiers: Vec<MSCallModifier>,

    pub parameters: Vec<Parameter<Variable>>,

    pub inlineAssembly: Option<InlineAssembly>,

    /// Function name
    ///
    /// May be anonymous if used inside a variable
    pub name: Option<Identifier>,
    // FIXME: dropping $.identifier aufter parameter list...
}

impl MSPointerModifier {
    pub fn from_str(rawValue: &str) -> Option<Self> {
        match rawValue {
            "_unaligned" | "__unaligned" => Some(Self::unaligned),
            "__restrict" => Some(Self::restrict),
            "__uptr" => Some(Self::unsigned),
            "__sptr" => Some(Self::signed),
            "__based" => Some(Self::based(Vec::with_capacity(0))),
            _ => None,
        }
    }
}

impl fmt::Display for MSPointerModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::unaligned => write!(f, "__unaligned"),
            Self::restrict => write!(f, "__restrict"),
            Self::unsigned => write!(f, "__uptr"),
            Self::signed => write!(f, "__sptr"),
            Self::based(args) => {
                write!(f, "__based({})", args.join(", "))
            }
        }
    }
}

impl fmt::Display for MSDeclModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::declspec(identifier) => write!(f, "__declspec({identifier})"),
        }
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::attribute(attr) => write!(f, "{attr}"),
            Self::standardAttribute(attr) => write!(f, "{attr}"),
            Self::storage(storage) => write!(f, "{storage}"),
            Self::msSpecificModifier(modifier) => write!(f, "{modifier}"),
        }
    }
}

impl fmt::Display for DeclarationQualifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::constexpr => write!(f, "constexpr"),
            Self::extension => write!(f, "__extension__"),
            Self::alignas(expression) => write!(f, "alignas({})", expression),
        }
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "__attribute__(({}))", self.arguments.join(", "))
    }
}

impl fmt::Display for StandardAttribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[[{}{}]]",
            match self.prefix.as_ref() {
                Some(p) => p.clone() + "::",
                None => "".to_string()
            },
            self.name
        )
    }
}


impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::function(x) => write!(f, "{x}"),
            Self::variable(x) => write!(f, "{x}"),
            Self::typeDefinition(x) => write!(f, "{x}"),
            Self::structure(x) => write!(f, "struct {x}"),
            Self::union(x) => write!(f, "union {x}"),
            Self::enumeration(x) => write!(f, "{x}"),
            Self::macroDefintion(x) => write!(f, "{x}"),
            // Self::header(x) => write!(f, "{x}"),
            // Self::source(x) => write!(f, "{x}"),
        }
    }
}

fn fmt_separated_list<T: fmt::Display>(
    f: &mut fmt::Formatter,
    items: &[T],
    separator: &str,
    prefix: &str,
    suffix: &str,
) -> fmt::Result {
    if items.is_empty() {
        return Ok(())
    }

    write!(f, "{prefix}")?;

    let mut iter = items.iter();
    if let Some(first) = iter.next() {
        write!(f, "{first}")?;
    }

    for item in iter {
        write!(f, "{separator}{item}")?;
    }
    write!(f, "{suffix}")?;

    Ok(())
}

impl fmt::Display for TypeDecl {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::array(array) => write!(f, "{array}"),
            Self::enumeration(enumeration) => write!(f, "{enumeration}"),
            Self::structure(structure) => write!(f, "struct {structure}"),
            Self::union(union) => write!(f, "union {union}"),
            Self::function(function) => write!(f, "{function}"),
            Self::pointer(pointer) => write!(f, "{pointer}"),
            Self::macroCall(macroCall) => write!(f, "{macroCall}"),
            Self::named(named, sized) =>  {
                fmt_separated_list(f, sized, " ", "", " ")?;
                write!(f, "{named}")
            }
        }
    }
}

impl fmt::Display for CType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_separated_list(f, &self.qualifiers, " ", "", " ")?;
        write!(f, "{}", self.decl)
    }
}

impl fmt::Display for NamedType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::custom(identifier) => write!(f, "{identifier}"),
            Self::primitive(primitive) => write!(f, "{s}", s=primitive.as_ref()),
        }
    }
}


impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {        
        fmt_separated_list(f, &self.modifiers, " ", "", " ")?;
        fmt_separated_list(f, &self.declQualifiers, " ", "", " ")?;
        fmt_separated_list(f, &self.functionQualifiers, " ", "", " ")?;
        write!(f, "{}(", self.name.clone().unwrap_or(String::new()))?;
        fmt_separated_list(f, &self.parameters, ", ", "", "")?;
        write!(f, ") -> {}", self.returnType)
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt_separated_list(f, &self.modifiers, " ", "", " ")?;
        fmt_separated_list(f, &self.qualifiers, " ", "", " ")?;
        write!(f, "var {}: {}", 
            self.identifier.clone().unwrap_or("_".to_string()), 
            self.cType)?;

        if let Some(attached) = self.attachedExpression.clone() {
            match attached {
                AttachedExpression::value(expr) => write!(f, " = {expr}")?,
                AttachedExpression::bitWidth(expr) => write!(f, " : ({expr} bits)")?,
            };
        }

        Ok(())
    }
}

impl<T> fmt::Display for Parameter<T> where T: fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::regular(variable) => write!(f, "{variable}"),
            Self::variadic(identifier) => write!(f, "{s}...", s=identifier.clone().unwrap_or(String::new()))
        }
    }
}

impl fmt::Display for Array {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        fmt_separated_list(f, &self.qualifiers, " ", "", " ")?;
        if let Some(countExpr) = self.count.clone() {
            write!(f, "{countExpr} x ")?;
        }
        write!(f, "{}]", self.elementType)
    }
}

impl fmt::Display for Enum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "union {s} {{", s=self.name.clone().unwrap_or("<anon>".to_string()))?;
        fmt_separated_list(f, &self.cases, "; ", " ", " ")?;
        write!(f, "}}")
    }
}

impl fmt::Display for EnumCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{s}", s=self.name)?;

        if let Some(value) = self.value.clone() {
            write!(f, " = {value}")?;
        } 
        
        Ok(())
    }
}

impl fmt::Display for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{s} {{", s=self.name.clone().unwrap_or("<anon>".to_string()))?;
        fmt_separated_list(f, &self.members, "; ", " ", " ")?;
        write!(f, "}}")
    }
}

impl fmt::Display for PreprocessorCall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{n}", n=self.directive)?;
        fmt_separated_list(f, &self.arguments, ", ", "(", ")")?;
        Ok(())
    }
}

impl fmt::Display for MacroDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#define {n}", n=self.name)?;
        if !self.parameters.is_empty() {
            write!(f, "(")?;
            fmt_separated_list(f, &self.parameters, ", ", "", "")?;
            write!(f, ")")?;

            if let Some(value) = &self.value {
                write!(f, " {value}");
            }
        }
        Ok(())
    }
}

impl fmt::Display for Pointer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_separated_list(f, &self.msSpecificModifiers, " ", "", " ")?;
        fmt_separated_list(f, &self.qualifiers, " ", "", " ")?;
        write!(f, "Pointer<{t}>", t=self.pointeeType)
    }
}

impl fmt::Display for TypeDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{s} := ", s=self.name)?;
        fmt_separated_list(f, &self.qualifiers, " ", "", " ")?;
        write!(f, "{t}", t=self.originalType)?;
        fmt_separated_list(f, &self.attributes, " ", " ", "")
    }
}

impl fmt::Display for TranslationUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<translation unit>")
    }
}