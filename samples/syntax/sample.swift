/// Swift Syntax Highlighting Test
/// A document parser with protocol-oriented design.

import Foundation

// MARK: - Protocols

protocol Parseable {
    associatedtype Output
    func parse(_ input: String) throws -> Output
}

protocol Cacheable: AnyObject {
    var cacheKey: String { get }
    var expiresAt: Date? { get }
    func invalidate()
}

// MARK: - Error Types

enum ParserError: Error, LocalizedError {
    case invalidInput(String)
    case unexpectedToken(expected: String, found: String)
    case nestingTooDeep(maxDepth: Int)
    case timeout(after: TimeInterval)

    var errorDescription: String? {
        switch self {
        case .invalidInput(let detail):
            return "Invalid input: \(detail)"
        case .unexpectedToken(let expected, let found):
            return "Expected '\(expected)', found '\(found)'"
        case .nestingTooDeep(let max):
            return "Nesting exceeds maximum depth of \(max)"
        case .timeout(let interval):
            return "Operation timed out after \(interval)s"
        }
    }
}

// MARK: - Models

struct Token: Hashable, Sendable {
    enum Kind: String, CaseIterable {
        case text, heading, bold, italic, code
        case link, image, list, blockquote
    }

    let kind: Kind
    let content: String
    let range: Range<String.Index>
    let children: [Token]

    init(kind: Kind, content: String, range: Range<String.Index>, children: [Token] = []) {
        self.kind = kind
        self.content = content
        self.range = range
        self.children = children
    }
}

struct Document {
    let title: String
    let tokens: [Token]
    let metadata: [String: Any]
    let createdAt: Date

    var wordCount: Int {
        tokens.reduce(0) { count, token in
            count + token.content.split(separator: " ").count
        }
    }

    var headings: [Token] {
        tokens.filter { $0.kind == .heading }
    }
}

// MARK: - Generic Cache

final class LRUCache<Key: Hashable, Value>: Cacheable {
    private struct Entry {
        let value: Value
        let key: Key
        var lastAccess: Date
    }

    private var entries: [Key: Entry] = [:]
    private let capacity: Int
    private let lock = NSLock()

    let cacheKey: String
    var expiresAt: Date?

    init(capacity: Int = 100, name: String = "default") {
        self.capacity = capacity
        self.cacheKey = "cache.\(name)"
    }

    subscript(key: Key) -> Value? {
        get {
            lock.lock()
            defer { lock.unlock() }

            guard var entry = entries[key] else { return nil }
            entry.lastAccess = Date()
            entries[key] = entry
            return entry.value
        }
        set {
            lock.lock()
            defer { lock.unlock() }

            if let value = newValue {
                entries[key] = Entry(value: value, key: key, lastAccess: Date())
                evictIfNeeded()
            } else {
                entries.removeValue(forKey: key)
            }
        }
    }

    func invalidate() {
        lock.lock()
        defer { lock.unlock() }
        entries.removeAll()
    }

    private func evictIfNeeded() {
        guard entries.count > capacity else { return }
        let sorted = entries.sorted { $0.value.lastAccess < $1.value.lastAccess }
        let toRemove = entries.count - capacity
        for (key, _) in sorted.prefix(toRemove) {
            entries.removeValue(forKey: key)
        }
    }
}

// MARK: - Parser Implementation

class MarkdownParser: Parseable {
    typealias Output = Document

    private let maxDepth: Int
    private let cache: LRUCache<String, Document>
    private static let headingPattern = /^(#{1,6})\s+(.+)$/

    init(maxDepth: Int = 10) {
        self.maxDepth = maxDepth
        self.cache = LRUCache(capacity: 50, name: "markdown")
    }

    func parse(_ input: String) throws -> Document {
        let cacheKey = String(input.prefix(100).hashValue, radix: 16)
        if let cached = cache[cacheKey] {
            return cached
        }

        guard !input.isEmpty else {
            throw ParserError.invalidInput("Empty document")
        }

        var tokens: [Token] = []
        let lines = input.split(separator: "\n", omittingEmptySubsequences: false)

        for line in lines {
            let lineStr = String(line)
            let range = lineStr.startIndex..<lineStr.endIndex

            if let match = lineStr.firstMatch(of: Self.headingPattern) {
                let level = match.1.count
                let text = String(match.2)
                tokens.append(Token(kind: .heading, content: text, range: range))
                _ = level // used for heading level
            } else if lineStr.hasPrefix("> ") {
                let content = String(lineStr.dropFirst(2))
                tokens.append(Token(kind: .blockquote, content: content, range: range))
            } else if lineStr.hasPrefix("- ") || lineStr.hasPrefix("* ") {
                let content = String(lineStr.dropFirst(2))
                tokens.append(Token(kind: .list, content: content, range: range))
            } else {
                tokens.append(Token(kind: .text, content: lineStr, range: range))
            }
        }

        let title = tokens.first { $0.kind == .heading }?.content ?? "Untitled"
        let doc = Document(
            title: title,
            tokens: tokens,
            metadata: ["parser": "MarkdownParser", "version": "1.0"],
            createdAt: Date()
        )

        cache[cacheKey] = doc
        return doc
    }
}

// MARK: - Async Processing

actor DocumentProcessor {
    private var processed: [String: Document] = [:]
    private let parser = MarkdownParser()

    func process(_ input: String, id: String) async throws -> Document {
        if let existing = processed[id] {
            return existing
        }

        let doc = try parser.parse(input)
        processed[id] = doc
        return doc
    }

    func batchProcess(_ inputs: [(id: String, content: String)]) async -> [Result<Document, Error>] {
        await withTaskGroup(of: (Int, Result<Document, Error>).self) { group in
            for (index, input) in inputs.enumerated() {
                group.addTask {
                    do {
                        let doc = try await self.process(input.content, id: input.id)
                        return (index, .success(doc))
                    } catch {
                        return (index, .failure(error))
                    }
                }
            }

            var results = Array(repeating: Result<Document, Error>.failure(ParserError.invalidInput("pending")),
                                count: inputs.count)
            for await (index, result) in group {
                results[index] = result
            }
            return results
        }
    }
}

// MARK: - Extensions

extension String {
    var isBlank: Bool { allSatisfy(\.isWhitespace) }

    func truncated(to length: Int, trailing: String = "…") -> String {
        guard count > length else { return self }
        return prefix(length) + trailing
    }
}

extension Array where Element == Token {
    func ofKind(_ kind: Token.Kind) -> [Token] {
        filter { $0.kind == kind }
    }

    var textContent: String {
        map(\.content).joined(separator: " ")
    }
}

// MARK: - Property Wrappers

@propertyWrapper
struct Clamped<Value: Comparable> {
    private var value: Value
    private let range: ClosedRange<Value>

    var wrappedValue: Value {
        get { value }
        set { value = min(max(newValue, range.lowerBound), range.upperBound) }
    }

    init(wrappedValue: Value, _ range: ClosedRange<Value>) {
        self.range = range
        self.value = min(max(wrappedValue, range.lowerBound), range.upperBound)
    }
}

// MARK: - Usage

struct EditorConfig {
    @Clamped(1...400) var fontSize: Int = 14
    @Clamped(40...400) var lineWidth: Int = 80
    var theme: String = "dark"
    var showLineNumbers: Bool = true
}
