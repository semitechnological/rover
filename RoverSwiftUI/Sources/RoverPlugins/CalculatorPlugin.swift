import Foundation
import RoverCore

/// Evaluates math expressions via `NSExpression`.
@MainActor
public final class CalculatorPlugin: RoverPlugin {
    public let id = "builtin.calculator"
    public let name = "Calculator"

    /// Characters allowed in an expression — anything else means it's not a math query.
    private static let allowedChars = CharacterSet.decimalDigits
        .union(CharacterSet(charactersIn: "+-*/.()^ eE,"))

    public init() {}

    public func query(_ query: String) -> [PickerItem] { [] }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        return .copyToClipboard(itemID)
    }

    public func inlineCard(for query: String) -> InlineCard? {
        guard let result = evaluate(query) else { return nil }
        return InlineCard(kind: .calculator, content: result)
    }

    // MARK: - Private

    private func evaluate(_ expr: String) -> String? {
        let sanitised = expr
            .trimmingCharacters(in: .whitespaces)
            .replacingOccurrences(of: "^", with: "**")

        // Must contain a digit to be a math expression.
        guard sanitised.contains(where: \.isNumber) else { return nil }

        // Reject anything outside safe math characters to avoid NSExpression exceptions.
        // NSExpression(format:) raises an ObjC exception on invalid input; we can't
        // catch those in Swift, so we validate strictly before calling it.
        guard sanitised.unicodeScalars.allSatisfy({ Self.allowedChars.contains($0) }) else {
            return nil
        }

        let nsExpr = NSExpression(format: sanitised)
        guard let value = nsExpr.expressionValue(with: nil, context: nil) as? NSNumber else {
            return nil
        }

        let d = value.doubleValue
        guard d.isFinite else { return nil }

        if d == d.rounded(), let i = Int(exactly: d) {
            return "\(i)"
        }
        return String(format: "%g", d)
    }
}
