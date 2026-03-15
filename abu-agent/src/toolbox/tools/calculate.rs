#[abu_macros::tool(
    struct_name = Calculator,
    description = "Calculate the result of a given formula.",
)]
fn calculate(
    #[arg(description="Numerical expression to compute the result of, in Python syntax.")]
    formula: &str
) -> std::result::Result<f64, meval::Error> {
    meval::eval_str(formula)
}

#[allow(unused)]
#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_calculate_add() {
        assert_eq!(
            Calculator::calculate("1 + 1").unwrap(),
            2.
        );

        assert_eq!(
            Calculator::calculate("1.1212 + 12.121").unwrap(),
            13.2422
        );
    }
}