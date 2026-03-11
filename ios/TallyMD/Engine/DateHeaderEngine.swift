import Foundation

/// Port of date-related logic from desktop's finished.rs.
enum DateHeaderEngine {

    // MARK: - Fill Empty Days

    /// Fill empty day headers between oldest existing date and today.
    static func fillEmptyDays(text: String, today: Date, dateFormat: String) -> String {
        let lines = text.splitLines()
        var dates: [Date] = []

        for line in lines {
            if line.hasPrefix("## ") {
                let dateStr = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                if let date = DateParsing.parseFlexible(dateStr) {
                    dates.append(date)
                }
            }
        }

        guard !dates.isEmpty else { return text }

        dates.sort()
        let oldest = dates.first!
        let newest = today > dates.last! ? today : dates.last!

        var resultLines = lines
        let calendar = Calendar.current

        var date = oldest
        while date <= newest {
            if !dates.contains(where: { calendar.isDate($0, inSameDayAs: date) }) {
                let formatted = DateParsing.format(date: date, strftimeFormat: dateFormat)
                let header = "## \(formatted)"
                let pos = findDateInsertPosition(lines: resultLines, date: date)
                resultLines.insert("", at: pos)
                resultLines.insert(header, at: pos)
            }
            date = calendar.date(byAdding: .day, value: 1, to: date)!
        }

        return resultLines.joined(separator: "\n")
    }

    // MARK: - Insert Into Finished

    /// Insert an entry under today's date header in done.md.
    static func insertIntoFinished(
        finishedText: String, today: Date, entry: String, dateFormat: String
    ) -> String {
        var lines: [String]
        if finishedText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            lines = []
        } else {
            lines = finishedText.splitLines()
        }

        let formatted = DateParsing.format(date: today, strftimeFormat: dateFormat)
        let dateHeader = "## \(formatted)"
        let calendar = Calendar.current

        // Find today's header by parsing dates
        let headerIdx = lines.firstIndex { line in
            guard line.hasPrefix("## ") else { return false }
            let ds = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
            guard let d = DateParsing.parseFlexible(ds) else { return false }
            return calendar.isDate(d, inSameDayAs: today)
        }

        if let idx = headerIdx {
            // Update header to current format
            lines[idx] = dateHeader
            var insertAt = idx + 1
            while insertAt < lines.count {
                let trimmed = lines[insertAt].trimmingCharacters(in: .whitespaces)
                if trimmed.isEmpty || trimmed.hasPrefix("## ") { break }
                insertAt += 1
            }
            lines.insert(entry, at: insertAt)
        } else if lines.isEmpty {
            lines.append(dateHeader)
            lines.append(entry)
        } else {
            // Insert at top (newest first)
            lines.insert("", at: 0)
            lines.insert(entry, at: 0)
            lines.insert(dateHeader, at: 0)
        }

        return lines.joined(separator: "\n")
    }

    // MARK: - Reformat Date Headers

    /// Rewrite all `## <date>` headers using the new format.
    static func reformatDateHeaders(text: String, newFormat: String) -> String {
        let lines = text.splitLines()
        var result: [String] = []

        for line in lines {
            if line.hasPrefix("## ") {
                let dateStr = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                if let date = DateParsing.parseFlexible(dateStr) {
                    let formatted = DateParsing.format(date: date, strftimeFormat: newFormat)
                    result.append("## \(formatted)")
                    continue
                }
            }
            result.append(line)
        }

        return result.joined(separator: "\n")
    }

    // MARK: - Helpers

    private static func findDateInsertPosition(lines: [String], date: Date) -> Int {
        let calendar = Calendar.current
        for (i, line) in lines.enumerated() {
            if line.hasPrefix("## ") {
                let dateStr = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                if let existing = DateParsing.parseFlexible(dateStr) {
                    if date > existing || calendar.isDate(date, inSameDayAs: existing) {
                        return i
                    }
                }
            }
        }
        return lines.count
    }
}
