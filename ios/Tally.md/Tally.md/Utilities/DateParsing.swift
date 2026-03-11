import Foundation

/// Date parsing and formatting utilities matching the desktop's 6 supported formats.
enum DateParsing {

    /// strftime -> Swift DateFormatter format mapping.
    private static let formatMap: [(strftime: String, swift: String)] = [
        ("%Y-%m-%d", "yyyy-MM-dd"),
        ("%d-%m-%Y", "dd-MM-yyyy"),
        ("%d/%m/%Y", "dd/MM/yyyy"),
        ("%m/%d/%Y", "MM/dd/yyyy"),
        ("%B %d, %Y", "MMMM dd, yyyy"),
        ("%d %B %Y", "dd MMMM yyyy"),
    ]

    /// All Swift date format strings for flexible parsing.
    private static let swiftFormats = formatMap.map(\.swift)

    /// Convert a strftime format string to Swift DateFormatter format.
    static func swiftFormat(from strftime: String) -> String {
        formatMap.first { $0.strftime == strftime }?.swift ?? "yyyy-MM-dd"
    }

    /// Try parsing a date string in any of the 6 supported formats.
    static func parseFlexible(_ string: String) -> Date? {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")

        for fmt in swiftFormats {
            formatter.dateFormat = fmt
            if let date = formatter.date(from: string) {
                return date
            }
        }
        return nil
    }

    /// Format a date using a strftime-style format string.
    static func format(date: Date, strftimeFormat: String) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.dateFormat = swiftFormat(from: strftimeFormat)
        return formatter.string(from: date)
    }

    /// Get a preview string of today's date in a given strftime format.
    static func preview(strftimeFormat: String) -> String {
        format(date: Date(), strftimeFormat: strftimeFormat)
    }

    /// All supported strftime format strings.
    static let allFormats = formatMap.map(\.strftime)
}
