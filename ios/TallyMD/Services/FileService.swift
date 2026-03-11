import Foundation

/// Reads and writes todo.md, today.md, done.md, and settings.json.
enum FileService {

    /// The active directory based on storage mode.
    static func todosDirectory(settings: AppSettings) -> URL {
        if settings.storageMode == "git" && !settings.gitRepoName.isEmpty {
            return documentsDirectory
                .appendingPathComponent("repos")
                .appendingPathComponent(settings.gitRepoName)
        }
        return documentsDirectory.appendingPathComponent("todos")
    }

    /// App's Documents directory.
    static var documentsDirectory: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    }

    /// Settings file in the app's config area.
    static var settingsPath: URL {
        documentsDirectory.appendingPathComponent("settings.json")
    }

    // MARK: - File Operations

    static func loadFiles(settings: AppSettings) -> (todo: String, today: String, done: String) {
        let dir = todosDirectory(settings: settings)
        ensureDirectory(dir)

        let todo = readFile(dir.appendingPathComponent("todo.md"))
        let today = readFile(dir.appendingPathComponent("today.md"))
        let done = readFile(dir.appendingPathComponent("done.md"))

        return (todo, today, done)
    }

    static func saveFiles(
        todo: String, today: String, done: String, settings: AppSettings
    ) {
        let dir = todosDirectory(settings: settings)
        ensureDirectory(dir)

        writeFile(dir.appendingPathComponent("todo.md"), content: todo)
        writeFile(dir.appendingPathComponent("today.md"), content: today)
        writeFile(dir.appendingPathComponent("done.md"), content: done)
    }

    // MARK: - Settings

    static func loadSettings() -> AppSettings {
        guard let data = try? Data(contentsOf: settingsPath),
              let settings = try? JSONDecoder().decode(AppSettings.self, from: data)
        else {
            return .default
        }
        return settings
    }

    static func saveSettings(_ settings: AppSettings) {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(settings) else { return }
        try? data.write(to: settingsPath)

        // Also save to repo directory
        let dir = todosDirectory(settings: settings)
        if FileManager.default.fileExists(atPath: dir.path) {
            try? data.write(to: dir.appendingPathComponent("settings.json"))
        }
    }

    // MARK: - Helpers

    private static func ensureDirectory(_ url: URL) {
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    }

    private static func readFile(_ url: URL) -> String {
        (try? String(contentsOf: url, encoding: .utf8)) ?? ""
    }

    private static func writeFile(_ url: URL, content: String) {
        try? content.write(to: url, atomically: true, encoding: .utf8)
    }
}
