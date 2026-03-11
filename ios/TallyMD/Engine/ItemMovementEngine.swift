import Foundation

/// Pure-function port of desktop's finished.rs item movement logic.
/// All functions take strings in, return strings out — no side effects.
enum ItemMovementEngine {

    // MARK: - Move Forward (todo -> today)

    /// Move the `- ` item at `cursorLine` from source to target, adding a breadcrumb.
    /// Returns `(newSource, newTarget)` or nil if the line isn't a `- ` item.
    static func moveItemForward(
        source: String, target: String, cursorLine: Int
    ) -> (String, String)? {
        let sourceLines = source.splitLines()
        guard cursorLine < sourceLines.count else { return nil }

        let line = sourceLines[cursorLine]
        let trimmed = line.trimLeading()
        guard trimmed.hasPrefix("- ") else { return nil }

        let text = String(trimmed.dropFirst(2))
        let breadcrumb = breadcrumbFor(lines: sourceLines, lineIdx: cursorLine)

        let entry: String
        if breadcrumb.isEmpty {
            entry = "- \(text)"
        } else {
            entry = "- \(text) (\(breadcrumb.joined(separator: " > ")))"
        }

        var newSource = sourceLines
        newSource.remove(at: cursorLine)
        if newSource.isEmpty { newSource.append("") }

        var targetStr = target
        if targetStr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            targetStr = entry
        } else {
            targetStr += "\n" + entry
        }

        return (newSource.joined(separator: "\n"), targetStr)
    }

    // MARK: - Move Back (today -> todo)

    /// Move the `- ` item at `cursorLine` from source back to target, stripping breadcrumb.
    /// Returns `(newSource, newTarget)` or nil.
    static func moveItemBack(
        source: String, target: String, cursorLine: Int
    ) -> (String, String)? {
        let sourceLines = source.splitLines()
        guard cursorLine < sourceLines.count else { return nil }

        let line = sourceLines[cursorLine]
        let trimmed = line.trimLeading()
        guard trimmed.hasPrefix("- ") else { return nil }

        let text = String(trimmed.dropFirst(2))
        let (cleanText, breadcrumb) = extractBreadcrumb(from: text)

        var newSource = sourceLines
        newSource.remove(at: cursorLine)
        if newSource.isEmpty { newSource.append("") }

        let newLine = "- \(cleanText)"
        var targetLines: [String]
        if target.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            targetLines = []
        } else {
            targetLines = target.splitLines()
        }

        if let crumb = breadcrumb, let pos = findParentPosition(lines: targetLines, breadcrumb: crumb) {
            targetLines.insert(newLine, at: pos)
        } else {
            targetLines.append(newLine)
        }

        return (newSource.joined(separator: "\n"), targetLines.joined(separator: "\n"))
    }

    // MARK: - Complete Item (todo/today -> done with date header)

    /// Move item to done.md under today's date header.
    static func completeItem(
        source: String, target: String, cursorLine: Int,
        today: Date, dateFormat: String
    ) -> (String, String)? {
        let sourceLines = source.splitLines()
        guard cursorLine < sourceLines.count else { return nil }

        let line = sourceLines[cursorLine]
        let trimmed = line.trimLeading()
        guard trimmed.hasPrefix("- ") else { return nil }

        let text = String(trimmed.dropFirst(2))
        let breadcrumb = breadcrumbFor(lines: sourceLines, lineIdx: cursorLine)

        let entry: String
        if breadcrumb.isEmpty {
            entry = "- \(text)"
        } else {
            entry = "- \(text) (\(breadcrumb.joined(separator: " > ")))"
        }

        var newSource = sourceLines
        newSource.remove(at: cursorLine)
        if newSource.isEmpty { newSource.append("") }

        let newTarget = DateHeaderEngine.insertIntoFinished(
            finishedText: target, today: today, entry: entry, dateFormat: dateFormat
        )

        return (newSource.joined(separator: "\n"), newTarget)
    }

    // MARK: - Recover Item (done -> todo)

    /// Recover item from done back to todo, stripping breadcrumb.
    static func recoverItem(
        source: String, target: String, cursorLine: Int
    ) -> (String, String)? {
        let sourceLines = source.splitLines()
        guard cursorLine < sourceLines.count else { return nil }

        let line = sourceLines[cursorLine]
        let trimmed = line.trimLeading()
        guard trimmed.hasPrefix("- ") else { return nil }

        let text = String(trimmed.dropFirst(2))
        let (cleanText, breadcrumb) = extractBreadcrumb(from: text)

        var newSource = sourceLines
        newSource.remove(at: cursorLine)
        if newSource.isEmpty { newSource.append("") }

        let newLine = "- \(cleanText)"
        var targetLines: [String]
        if target.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            targetLines = []
        } else {
            targetLines = target.splitLines()
        }

        if let crumb = breadcrumb, let pos = findParentPosition(lines: targetLines, breadcrumb: crumb) {
            targetLines.insert(newLine, at: pos)
        } else {
            targetLines.append(newLine)
        }

        return (newSource.joined(separator: "\n"), targetLines.joined(separator: "\n"))
    }

    // MARK: - Breadcrumb

    /// Extract parent breadcrumb trail for a `- ` item.
    static func breadcrumbFor(lines: [String], lineIdx: Int) -> [String] {
        let line = lines[lineIdx]
        let indent = line.leadingSpaceCount()

        if indent == 0 {
            // Look for heading above
            for i in stride(from: lineIdx - 1, through: 0, by: -1) {
                let l = lines[i].trimLeading()
                if l.hasPrefix("# ") || l.hasPrefix("## ") || l.hasPrefix("### ") {
                    let text = l.drop(while: { $0 == "#" }).trimmingCharacters(in: .whitespaces)
                    return [text]
                }
            }
            return []
        }

        var crumbs: [String] = []
        var currentIndent = indent

        for i in stride(from: lineIdx - 1, through: 0, by: -1) {
            let l = lines[i]
            let lIndent = l.leadingSpaceCount()
            let trimmed = l.trimLeading()

            if lIndent < currentIndent && trimmed.hasPrefix("- ") {
                let text = String(trimmed.dropFirst(2))
                crumbs.append(text)
                currentIndent = lIndent
                if currentIndent == 0 { break }
            }

            if trimmed.hasPrefix("#") && currentIndent > 0 {
                let text = trimmed.drop(while: { $0 == "#" }).trimmingCharacters(in: .whitespaces)
                crumbs.append(text)
                break
            }
        }

        crumbs.reverse()
        return crumbs
    }

    // MARK: - Helpers

    /// Extract breadcrumb from text like "Item text (Heading > Parent)".
    /// Returns (cleanText, breadcrumb) where breadcrumb is the string inside parens.
    static func extractBreadcrumb(from text: String) -> (String, String?) {
        guard let parenStart = text.range(of: " (", options: .backwards) else {
            return (text, nil)
        }
        guard text.hasSuffix(")") else {
            return (text, nil)
        }

        let crumbStart = text.index(parenStart.upperBound, offsetBy: 0)
        let crumbEnd = text.index(text.endIndex, offsetBy: -1)
        let crumb = String(text[crumbStart..<crumbEnd])
        let clean = String(text[text.startIndex..<parenStart.lowerBound])
        return (clean, crumb)
    }

    /// Find the position to insert an item below its parent heading/item in target.
    static func findParentPosition(lines: [String], breadcrumb: String) -> Int? {
        let parts = breadcrumb.components(separatedBy: " > ")
        let heading = parts[0]

        // Find the heading
        var headingIdx: Int?
        for (i, line) in lines.enumerated() {
            let trimmed = line.trimLeading()
            if trimmed.hasPrefix("#") {
                let text = trimmed.drop(while: { $0 == "#" }).trimmingCharacters(in: .whitespaces)
                if text == heading {
                    headingIdx = i
                    break
                }
            }
        }

        guard let hIdx = headingIdx else { return nil }

        // If breadcrumb has more parts, find the parent item
        if parts.count > 1 {
            let parentItem = parts.last!
            for i in (hIdx + 1)..<lines.count {
                let trimmed = lines[i].trimLeading()
                if trimmed.hasPrefix("#") { break }
                if trimmed.hasPrefix("- ") {
                    let itemText = String(trimmed.dropFirst(2))
                    if itemText == parentItem {
                        let parentIndent = lines[i].leadingSpaceCount()
                        var insertAt = i + 1
                        while insertAt < lines.count {
                            let l = lines[insertAt]
                            let lIndent = l.leadingSpaceCount()
                            if l.trimmingCharacters(in: .whitespaces).isEmpty || lIndent <= parentIndent {
                                break
                            }
                            insertAt += 1
                        }
                        return insertAt
                    }
                }
            }
        }

        // Insert at end of heading's section
        var insertAt = hIdx + 1
        while insertAt < lines.count {
            let trimmed = lines[insertAt].trimLeading()
            if trimmed.hasPrefix("#") { break }
            insertAt += 1
        }
        return insertAt
    }
}

// MARK: - String Helpers

extension String {
    func splitLines() -> [String] {
        self.components(separatedBy: "\n")
    }

    func trimLeading() -> String {
        var idx = startIndex
        while idx < endIndex && self[idx] == " " {
            idx = index(after: idx)
        }
        return String(self[idx...])
    }

    func leadingSpaceCount() -> Int {
        var count = 0
        for ch in self {
            if ch == " " { count += 1 } else { break }
        }
        return count
    }
}
