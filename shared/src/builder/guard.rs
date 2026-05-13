//! Guard-Mini-DSL fuer [`super::EventTrigger`].
//!
//! Phase 1.2: ein `EventTrigger` darf einen optionalen `guard` tragen, der
//! als Mini-Expression ueber `fields.*` formuliert ist. Beispiel:
//!
//! ```text
//! fields.status == "draft" && fields.price > 0
//! ```
//!
//! Diese Datei stellt drei Dinge bereit:
//!   1. den Wire-Typ [`GuardExpr`] (transparent als String serialisiert),
//!   2. einen Recursive-Descent-Parser ([`parse`]), der den String in einen
//!      [`GuardAst`] uebersetzt,
//!   3. einen einfachen Evaluator ([`GuardAst::evaluate`]), der den AST
//!      gegen ein `serde_json::Map`-Feld-Set auswertet.
//!
//! Der Parser deckt bewusst nur die Grammatik ab, die der Roadmap-Vertrag
//! nennt (boolsche Verknuepfungen, Vergleiche, Klammern, Literale,
//! Field-Refs). Operatoren wie Arithmetik, Funktionen oder `in`-Operatoren
//! sind nicht vorgesehen — sollte das benoetigt werden, ist eine
//! Erweiterung lokal moeglich, ohne das Wire-Format zu aendern.

use serde::{Deserialize, Serialize};

/// String-Form des Guard-Ausdrucks. Wird transparent serialisiert, damit
/// das Wire-Format der reine Source-String bleibt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct GuardExpr(pub String);

impl GuardExpr {
    /// Konstruiert einen Guard aus einer String-aehnlichen Quelle.
    pub fn new(src: impl Into<String>) -> Self {
        GuardExpr(src.into())
    }

    /// Quelltext des Guards.
    pub fn source(&self) -> &str {
        &self.0
    }

    /// Parst den Guard zu einem [`GuardAst`].
    pub fn parse(&self) -> Result<GuardAst, GuardError> {
        parse(&self.0)
    }
}

impl<S> From<S> for GuardExpr
where
    S: Into<String>,
{
    fn from(src: S) -> Self {
        GuardExpr(src.into())
    }
}

// =============================================================================
// AST
// =============================================================================

/// Wurzel-Node des Guard-AST.
#[derive(Debug, Clone, PartialEq)]
pub enum GuardAst {
    Or(Box<GuardAst>, Box<GuardAst>),
    And(Box<GuardAst>, Box<GuardAst>),
    Not(Box<GuardAst>),
    Compare {
        left: Operand,
        op: CmpOp,
        right: Operand,
    },
    /// Nackter Operand als Bedingung (z.B. `fields.flag` oder `true`).
    /// Auswertung folgt JS-aehnlicher Truthiness.
    Bare(Operand),
}

/// Linker oder rechter Operand eines Vergleichs.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// `fields.foo.bar` — ein punktseparierter Pfad ab dem Reserved-Word `fields`.
    FieldRef(Vec<String>),
    /// Literal (String, Number, Bool, Null).
    Literal(Literal),
}

/// Literal-Wert in einem Guard-Ausdruck.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

/// Vergleichsoperator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Fehler beim Parsen oder Auswerten eines Guards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    UnexpectedChar { pos: usize, ch: char },
    UnexpectedEof,
    UnexpectedToken { pos: usize, found: String },
    UnterminatedString { pos: usize },
    InvalidNumber { pos: usize, src: String },
    /// Operand-Pfad beginnt nicht mit dem Reserved-Word `fields`.
    InvalidFieldRef { pos: usize, src: String },
}

impl core::fmt::Display for GuardError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GuardError::UnexpectedChar { pos, ch } => {
                write!(f, "unexpected character '{ch}' at pos {pos}")
            }
            GuardError::UnexpectedEof => write!(f, "unexpected end of input"),
            GuardError::UnexpectedToken { pos, found } => {
                write!(f, "unexpected token '{found}' at pos {pos}")
            }
            GuardError::UnterminatedString { pos } => {
                write!(f, "unterminated string starting at pos {pos}")
            }
            GuardError::InvalidNumber { pos, src } => {
                write!(f, "invalid number '{src}' at pos {pos}")
            }
            GuardError::InvalidFieldRef { pos, src } => {
                write!(f, "invalid field reference '{src}' at pos {pos} (must start with 'fields.')")
            }
        }
    }
}

impl std::error::Error for GuardError {}

// =============================================================================
// Parser
// =============================================================================

/// Parst einen Guard-Ausdruck.
pub fn parse(input: &str) -> Result<GuardAst, GuardError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser { tokens, pos: 0 };
    let ast = parser.parse_or()?;
    if parser.peek().is_some() {
        let tok = parser.peek().unwrap();
        return Err(GuardError::UnexpectedToken {
            pos: tok.start,
            found: tok.kind.describe(),
        });
    }
    Ok(ast)
}

#[derive(Debug, Clone, PartialEq)]
enum TokKind {
    LParen,
    RParen,
    And,
    Or,
    Not,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Dot,
    Ident(String),
    Str(String),
    Num(f64),
    True,
    False,
    Null,
}

impl TokKind {
    fn describe(&self) -> String {
        match self {
            TokKind::LParen => "(".into(),
            TokKind::RParen => ")".into(),
            TokKind::And => "&&".into(),
            TokKind::Or => "||".into(),
            TokKind::Not => "!".into(),
            TokKind::Eq => "==".into(),
            TokKind::Ne => "!=".into(),
            TokKind::Lt => "<".into(),
            TokKind::Le => "<=".into(),
            TokKind::Gt => ">".into(),
            TokKind::Ge => ">=".into(),
            TokKind::Dot => ".".into(),
            TokKind::Ident(s) => s.clone(),
            TokKind::Str(s) => format!("\"{s}\""),
            TokKind::Num(n) => n.to_string(),
            TokKind::True => "true".into(),
            TokKind::False => "false".into(),
            TokKind::Null => "null".into(),
        }
    }
}

#[derive(Debug, Clone)]
struct Token {
    kind: TokKind,
    start: usize,
}

fn tokenize(input: &str) -> Result<Vec<Token>, GuardError> {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        match c {
            '(' => {
                out.push(Token { kind: TokKind::LParen, start });
                i += 1;
            }
            ')' => {
                out.push(Token { kind: TokKind::RParen, start });
                i += 1;
            }
            '.' => {
                out.push(Token { kind: TokKind::Dot, start });
                i += 1;
            }
            '&' if i + 1 < bytes.len() && bytes[i + 1] == b'&' => {
                out.push(Token { kind: TokKind::And, start });
                i += 2;
            }
            '|' if i + 1 < bytes.len() && bytes[i + 1] == b'|' => {
                out.push(Token { kind: TokKind::Or, start });
                i += 2;
            }
            '=' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Token { kind: TokKind::Eq, start });
                i += 2;
            }
            '!' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Token { kind: TokKind::Ne, start });
                i += 2;
            }
            '!' => {
                out.push(Token { kind: TokKind::Not, start });
                i += 1;
            }
            '<' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Token { kind: TokKind::Le, start });
                i += 2;
            }
            '<' => {
                out.push(Token { kind: TokKind::Lt, start });
                i += 1;
            }
            '>' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => {
                out.push(Token { kind: TokKind::Ge, start });
                i += 2;
            }
            '>' => {
                out.push(Token { kind: TokKind::Gt, start });
                i += 1;
            }
            '"' | '\'' => {
                let quote = bytes[i];
                let str_start = i + 1;
                let mut j = str_start;
                let mut buf = String::new();
                let mut closed = false;
                while j < bytes.len() {
                    let b = bytes[j];
                    if b == b'\\' && j + 1 < bytes.len() {
                        let next = bytes[j + 1] as char;
                        let mapped = match next {
                            'n' => '\n',
                            't' => '\t',
                            '\\' => '\\',
                            '"' => '"',
                            '\'' => '\'',
                            other => other,
                        };
                        buf.push(mapped);
                        j += 2;
                    } else if b == quote {
                        closed = true;
                        j += 1;
                        break;
                    } else {
                        buf.push(b as char);
                        j += 1;
                    }
                }
                if !closed {
                    return Err(GuardError::UnterminatedString { pos: start });
                }
                out.push(Token { kind: TokKind::Str(buf), start });
                i = j;
            }
            c if c.is_ascii_digit() || (c == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit()) => {
                let mut j = i;
                if bytes[j] == b'-' {
                    j += 1;
                }
                while j < bytes.len() {
                    let b = bytes[j] as char;
                    if b.is_ascii_digit() || b == '.' {
                        j += 1;
                    } else {
                        break;
                    }
                }
                let src = &input[start..j];
                let n: f64 = src
                    .parse()
                    .map_err(|_| GuardError::InvalidNumber { pos: start, src: src.into() })?;
                out.push(Token { kind: TokKind::Num(n), start });
                i = j;
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let mut j = i;
                while j < bytes.len() {
                    let b = bytes[j] as char;
                    if b.is_ascii_alphanumeric() || b == '_' {
                        j += 1;
                    } else {
                        break;
                    }
                }
                let ident = &input[start..j];
                let kind = match ident {
                    "true" => TokKind::True,
                    "false" => TokKind::False,
                    "null" => TokKind::Null,
                    _ => TokKind::Ident(ident.into()),
                };
                out.push(Token { kind, start });
                i = j;
            }
            _ => return Err(GuardError::UnexpectedChar { pos: i, ch: c }),
        }
    }
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_or(&mut self) -> Result<GuardAst, GuardError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek().map(|t| &t.kind), Some(TokKind::Or)) {
            self.bump();
            let right = self.parse_and()?;
            left = GuardAst::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<GuardAst, GuardError> {
        let mut left = self.parse_unary()?;
        while matches!(self.peek().map(|t| &t.kind), Some(TokKind::And)) {
            self.bump();
            let right = self.parse_unary()?;
            left = GuardAst::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<GuardAst, GuardError> {
        if matches!(self.peek().map(|t| &t.kind), Some(TokKind::Not)) {
            self.bump();
            let inner = self.parse_unary()?;
            return Ok(GuardAst::Not(Box::new(inner)));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<GuardAst, GuardError> {
        // Klammergruppe: kann Boolean-Expression sein.
        if matches!(self.peek().map(|t| &t.kind), Some(TokKind::LParen)) {
            self.bump();
            let inner = self.parse_or()?;
            match self.bump() {
                Some(t) if t.kind == TokKind::RParen => return Ok(inner),
                Some(t) => {
                    return Err(GuardError::UnexpectedToken {
                        pos: t.start,
                        found: t.kind.describe(),
                    })
                }
                None => return Err(GuardError::UnexpectedEof),
            }
        }
        let left = self.parse_operand()?;
        let op = match self.peek().map(|t| &t.kind) {
            Some(TokKind::Eq) => Some(CmpOp::Eq),
            Some(TokKind::Ne) => Some(CmpOp::Ne),
            Some(TokKind::Lt) => Some(CmpOp::Lt),
            Some(TokKind::Le) => Some(CmpOp::Le),
            Some(TokKind::Gt) => Some(CmpOp::Gt),
            Some(TokKind::Ge) => Some(CmpOp::Ge),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let right = self.parse_operand()?;
            Ok(GuardAst::Compare { left, op, right })
        } else {
            Ok(GuardAst::Bare(left))
        }
    }

    fn parse_operand(&mut self) -> Result<Operand, GuardError> {
        match self.peek().map(|t| t.kind.clone()) {
            Some(TokKind::Str(s)) => {
                self.bump();
                Ok(Operand::Literal(Literal::Str(s)))
            }
            Some(TokKind::Num(n)) => {
                self.bump();
                Ok(Operand::Literal(Literal::Num(n)))
            }
            Some(TokKind::True) => {
                self.bump();
                Ok(Operand::Literal(Literal::Bool(true)))
            }
            Some(TokKind::False) => {
                self.bump();
                Ok(Operand::Literal(Literal::Bool(false)))
            }
            Some(TokKind::Null) => {
                self.bump();
                Ok(Operand::Literal(Literal::Null))
            }
            Some(TokKind::Ident(_)) => self.parse_field_ref(),
            Some(other) => {
                let pos = self.peek().map(|t| t.start).unwrap_or(0);
                Err(GuardError::UnexpectedToken {
                    pos,
                    found: other.describe(),
                })
            }
            None => Err(GuardError::UnexpectedEof),
        }
    }

    fn parse_field_ref(&mut self) -> Result<Operand, GuardError> {
        let head_tok = self.bump().ok_or(GuardError::UnexpectedEof)?;
        let head = match head_tok.kind {
            TokKind::Ident(s) => s,
            other => {
                return Err(GuardError::UnexpectedToken {
                    pos: head_tok.start,
                    found: other.describe(),
                })
            }
        };
        if head != "fields" {
            return Err(GuardError::InvalidFieldRef {
                pos: head_tok.start,
                src: head,
            });
        }
        let mut path = Vec::new();
        while matches!(self.peek().map(|t| &t.kind), Some(TokKind::Dot)) {
            self.bump();
            let seg = self.bump().ok_or(GuardError::UnexpectedEof)?;
            match seg.kind {
                TokKind::Ident(s) => path.push(s),
                other => {
                    return Err(GuardError::UnexpectedToken {
                        pos: seg.start,
                        found: other.describe(),
                    })
                }
            }
        }
        if path.is_empty() {
            return Err(GuardError::InvalidFieldRef {
                pos: head_tok.start,
                src: head,
            });
        }
        Ok(Operand::FieldRef(path))
    }
}

// =============================================================================
// Evaluator
// =============================================================================

impl GuardAst {
    /// Wertet den Guard gegen ein `fields`-Map aus.
    ///
    /// Vergleichs-Semantik:
    ///   - Strings und Zahlen: typgleicher Vergleich (`Eq`/`Ne` + Ordnung).
    ///   - Bools: nur `Eq`/`Ne`.
    ///   - Null: nur `Eq`/`Ne` gegen `Null`; jede andere Ordnung ergibt `false`.
    ///   - Typ-Mismatch (z.B. String vs. Zahl): `false` (`Ne` wird `true`).
    ///
    /// Truthiness fuer [`GuardAst::Bare`]:
    ///   - Bool: direkt
    ///   - Null: false
    ///   - Number: != 0
    ///   - String: nicht leer
    pub fn evaluate(&self, fields: &serde_json::Map<String, serde_json::Value>) -> bool {
        match self {
            GuardAst::Or(a, b) => a.evaluate(fields) || b.evaluate(fields),
            GuardAst::And(a, b) => a.evaluate(fields) && b.evaluate(fields),
            GuardAst::Not(a) => !a.evaluate(fields),
            GuardAst::Compare { left, op, right } => compare(left, *op, right, fields),
            GuardAst::Bare(op) => truthy(&resolve(op, fields)),
        }
    }
}

fn resolve(op: &Operand, fields: &serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    match op {
        Operand::Literal(Literal::Str(s)) => serde_json::Value::String(s.clone()),
        Operand::Literal(Literal::Num(n)) => serde_json::json!(n),
        Operand::Literal(Literal::Bool(b)) => serde_json::Value::Bool(*b),
        Operand::Literal(Literal::Null) => serde_json::Value::Null,
        Operand::FieldRef(path) => {
            // Erstes Segment greift in das `fields`-Map; folgende navigieren
            // durch verschachtelte Objekte.
            let mut cur: Option<&serde_json::Value> = path.first().and_then(|k| fields.get(k));
            for seg in path.iter().skip(1) {
                cur = cur.and_then(|v| v.get(seg));
            }
            cur.cloned().unwrap_or(serde_json::Value::Null)
        }
    }
}

fn compare(
    left: &Operand,
    op: CmpOp,
    right: &Operand,
    fields: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    let l = resolve(left, fields);
    let r = resolve(right, fields);
    use serde_json::Value as V;
    match (&l, &r) {
        (V::Null, V::Null) => matches!(op, CmpOp::Eq | CmpOp::Le | CmpOp::Ge),
        (V::Bool(a), V::Bool(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            _ => false,
        },
        (V::Number(a), V::Number(b)) => {
            let (Some(af), Some(bf)) = (a.as_f64(), b.as_f64()) else {
                return matches!(op, CmpOp::Ne);
            };
            match op {
                CmpOp::Eq => af == bf,
                CmpOp::Ne => af != bf,
                CmpOp::Lt => af < bf,
                CmpOp::Le => af <= bf,
                CmpOp::Gt => af > bf,
                CmpOp::Ge => af >= bf,
            }
        }
        (V::String(a), V::String(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
        },
        _ => matches!(op, CmpOp::Ne),
    }
}

fn truthy(v: &serde_json::Value) -> bool {
    use serde_json::Value as V;
    match v {
        V::Null => false,
        V::Bool(b) => *b,
        V::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        V::String(s) => !s.is_empty(),
        V::Array(a) => !a.is_empty(),
        V::Object(o) => !o.is_empty(),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        match v {
            serde_json::Value::Object(m) => m,
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn parses_simple_equality() {
        let ast = parse("fields.status == \"draft\"").unwrap();
        assert!(matches!(ast, GuardAst::Compare { op: CmpOp::Eq, .. }));
    }

    #[test]
    fn parses_and_chain() {
        let ast = parse("fields.status == \"draft\" && fields.price > 0").unwrap();
        assert!(matches!(ast, GuardAst::And(_, _)));
    }

    #[test]
    fn parses_parens_and_or() {
        let ast = parse("(fields.a == 1 || fields.b == 2) && fields.c").unwrap();
        assert!(matches!(ast, GuardAst::And(_, _)));
    }

    #[test]
    fn parses_negation() {
        let ast = parse("!fields.published").unwrap();
        assert!(matches!(ast, GuardAst::Not(_)));
    }

    #[test]
    fn rejects_non_fields_root() {
        let err = parse("other.foo == 1").unwrap_err();
        assert!(matches!(err, GuardError::InvalidFieldRef { .. }));
    }

    #[test]
    fn rejects_unterminated_string() {
        let err = parse("fields.x == \"oops").unwrap_err();
        assert!(matches!(err, GuardError::UnterminatedString { .. }));
    }

    #[test]
    fn rejects_trailing_garbage() {
        let err = parse("fields.x == 1 foo").unwrap_err();
        assert!(matches!(err, GuardError::UnexpectedToken { .. }));
    }

    #[test]
    fn evaluates_string_equality() {
        let ast = parse("fields.status == \"draft\"").unwrap();
        assert!(ast.evaluate(&map(json!({"status": "draft"}))));
        assert!(!ast.evaluate(&map(json!({"status": "published"}))));
    }

    #[test]
    fn evaluates_number_comparison() {
        let ast = parse("fields.price > 0").unwrap();
        assert!(ast.evaluate(&map(json!({"price": 5}))));
        assert!(!ast.evaluate(&map(json!({"price": 0}))));
        assert!(!ast.evaluate(&map(json!({"price": -1}))));
    }

    #[test]
    fn evaluates_and_or_combination() {
        let ast =
            parse("fields.status == \"draft\" && (fields.price > 0 || fields.featured)").unwrap();
        assert!(ast.evaluate(&map(json!({"status": "draft", "price": 10, "featured": false}))));
        assert!(ast.evaluate(&map(json!({"status": "draft", "price": 0, "featured": true}))));
        assert!(!ast.evaluate(&map(json!({"status": "draft", "price": 0, "featured": false}))));
        assert!(!ast.evaluate(&map(json!({"status": "published", "price": 10}))));
    }

    #[test]
    fn evaluates_nested_field_ref() {
        let ast = parse("fields.meta.tag == \"x\"").unwrap();
        assert!(ast.evaluate(&map(json!({"meta": {"tag": "x"}}))));
        assert!(!ast.evaluate(&map(json!({"meta": {"tag": "y"}}))));
        assert!(!ast.evaluate(&map(json!({})))); // missing -> null -> !=
    }

    #[test]
    fn evaluates_null_equality() {
        let ast = parse("fields.foo == null").unwrap();
        assert!(ast.evaluate(&map(json!({})))); // fehlend -> null
        assert!(ast.evaluate(&map(json!({"foo": null}))));
        assert!(!ast.evaluate(&map(json!({"foo": 1}))));
    }

    #[test]
    fn evaluates_bare_truthiness() {
        let ast = parse("fields.flag").unwrap();
        assert!(ast.evaluate(&map(json!({"flag": true}))));
        assert!(!ast.evaluate(&map(json!({"flag": false}))));
        assert!(!ast.evaluate(&map(json!({"flag": null}))));
        assert!(!ast.evaluate(&map(json!({})))); // missing -> null -> falsy
        assert!(ast.evaluate(&map(json!({"flag": "non-empty"}))));
        assert!(!ast.evaluate(&map(json!({"flag": ""}))));
        assert!(!ast.evaluate(&map(json!({"flag": 0}))));
        assert!(ast.evaluate(&map(json!({"flag": 1}))));
    }

    #[test]
    fn type_mismatch_is_false_for_eq() {
        let ast = parse("fields.x == 1").unwrap();
        assert!(!ast.evaluate(&map(json!({"x": "1"}))));
        let ne = parse("fields.x != 1").unwrap();
        assert!(ne.evaluate(&map(json!({"x": "1"}))));
    }

    #[test]
    fn negative_numbers_parse() {
        let ast = parse("fields.price >= -10.5").unwrap();
        assert!(ast.evaluate(&map(json!({"price": -10.5}))));
        assert!(ast.evaluate(&map(json!({"price": 0}))));
        assert!(!ast.evaluate(&map(json!({"price": -11}))));
    }
}
