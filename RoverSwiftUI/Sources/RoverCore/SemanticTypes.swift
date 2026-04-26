// Semantic search types — forwarded from the rover_swift UniFFI bindings.
//
// This file provides Swift-visible declarations that match the types exported
// from rover_swift/src/lib.rs. `build-swiftui.sh` regenerates rover_swift.swift
// which contains the real generated implementations; this file just makes the
// types available to RoverPlugins at compile time.
//
// If `rover_swift.swift` already contains these types (after a build-swiftui.sh
// run), the compiler will see both definitions and complain. In that case delete
// this file — its job is done.
//
// NOTE: Do NOT import rover_swiftFFI here; this file is compiled as part of
// RoverCore which already has the FFI module in scope via rover_swiftFFI target.

#if !ROVER_SWIFT_GENERATED  // set -DROVER_SWIFT_GENERATED once bindings are live

/// A document to batch-index.
public struct SemanticDoc: Sendable {
    public var id: String
    public var collection: String
    public var text: String
    public init(id: String, collection: String, text: String) {
        self.id = id; self.collection = collection; self.text = text
    }
}

/// One semantic search result.
public struct SemanticResult: Sendable {
    public var id: String
    public var collection: String
    public var text: String
    /// Cosine similarity in [-1, 1].
    public var score: Float
}

/// Semantic search index backed by fastembed on the Rust side.
public final class SemanticSearch: Sendable {
    // The actual implementation lives in rover_swift.swift (generated).
    // This stub lets RoverPlugins compile before build-swiftui.sh has run.
    public static func open() throws -> SemanticSearch {
        throw SemanticError.unavailable
    }
    public func upsertBatch(docs: [SemanticDoc]) throws {}
    public func search(query: String, collection: String, topK: UInt32) throws -> [SemanticResult] { [] }
    public func count() -> UInt32 { 0 }
}

public enum SemanticError: Error {
    case unavailable
}

#endif
