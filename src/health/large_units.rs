use super::FunctionMetrics;
use super::thresholds::ThresholdContext;
use super::types::LargeFunction;

pub(super) fn large_functions(
    functions: &[FunctionMetrics],
    threshold_context: &mut ThresholdContext,
) -> Vec<LargeFunction> {
    let mut findings = functions
        .iter()
        .filter_map(|function| {
            let line_count = function
                .end_line
                .saturating_sub(function.location.line)
                .saturating_add(1);
            let thresholds = threshold_context.unit_size_thresholds(function, line_count);
            let max_unit_size = thresholds.effective.max_unit_size?;
            (line_count > max_unit_size).then(|| LargeFunction {
                path: function.path.clone(),
                symbol: function.symbol.clone(),
                kind: function.kind,
                location: function.location,
                end_line: function.end_line,
                line_count,
                max_unit_size,
                threshold_source: thresholds.source,
                threshold_reason: thresholds.reason,
            })
        })
        .collect::<Vec<_>>();

    findings.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.line_count),
            &left.path,
            left.location.line,
            left.location.column,
            &left.symbol,
        )
            .cmp(&(
                std::cmp::Reverse(right.line_count),
                &right.path,
                right.location.line,
                right.location.column,
                &right.symbol,
            ))
    });
    findings
}
