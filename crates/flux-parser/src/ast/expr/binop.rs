/// A binary operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BinOp {
    /// `+`.
    Add,
    /// `-`.
    Sub,
    /// `*`.
    Mul,
    /// `/`.
    Div,
    /// `%`.
    Rem,
    /// `==`.
    Eq,
    /// `!=`.
    Ne,
    /// `<`.
    Lt,
    /// `>`.
    Gt,
    /// `<=`.
    Le,
    /// `>=`.
    Ge,
    /// `&&`.
    And,
    /// `||`.
    Or,
}

impl BinOp {
    /// Parses an operator from its source spelling.
    ///
    /// Returns `None` when `text` is not one of the operators in Appendix B.
    ///
    /// # Examples
    ///
    /// ```
    /// use flux_parser::BinOp;
    ///
    /// assert_eq!(BinOp::from_source("<="), Some(BinOp::Le));
    /// assert_eq!(BinOp::from_source("<>"), None);
    /// ```
    #[must_use]
    pub fn from_source(text: &str) -> Option<Self> {
        match text {
            "+" => Some(Self::Add),
            "-" => Some(Self::Sub),
            "*" => Some(Self::Mul),
            "/" => Some(Self::Div),
            "%" => Some(Self::Rem),
            "==" => Some(Self::Eq),
            "!=" => Some(Self::Ne),
            "<" => Some(Self::Lt),
            ">" => Some(Self::Gt),
            "<=" => Some(Self::Le),
            ">=" => Some(Self::Ge),
            "&&" => Some(Self::And),
            "||" => Some(Self::Or),
            _ => None,
        }
    }
}
