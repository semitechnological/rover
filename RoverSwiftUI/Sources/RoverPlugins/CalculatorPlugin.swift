import Foundation
import RoverCore

/// Evaluates math expressions via `NSExpression`.
@MainActor
public final class CalculatorPlugin: RoverPlugin {
    public let id = "builtin.calculator"
    public let name = "Calculator"

    public init() {}

    public func query(_ query: String) -> [PickerItem] { [] }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        // Copy the result to clipboard.
        return .copyToClipboard(itemID)
    }

    public func inlineCard(for query: String) -> InlineCard? {
        guard let result = evaluate(query) else { return nil }
        return InlineCard(kind: .calculator, content: result)
    }

    // MARK: - Private

    private func evaluate(_ expr: String) -> String? {
        // NSExpression handles basic arithmetic: +, -, *, /, (), ^, sqrt(), etc.
        let sanitised = expr
            .trimmingCharacters(in: .whitespaces)
            .replacingOccurrences(of: "^", with: "**")

        // Reject pure-word queries so we don't false-positive on app names.
        let hasDigit = sanitised.contains { $0.isNumber }
        guard hasDigit else { return nil }

        guard
            let nsExpr = try? NSExpression(format: sanitised),
            let value = nsExpr.expressionValue(with: nil, context: nil) as? NSNumber
        else { return nil }

        let d = value.doubleValue
        // Prefer integer display when result is whole.
        if d == d.rounded() && !d.isInfinite, let i = Int(exactly: d) {
            return "\(i)"
        }
        return String(format: "%g", d)
    }
}
