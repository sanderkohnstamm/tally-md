import Foundation

struct AppSettings: Codable, Equatable {
    var storageMode: String
    var localPath: String
    var gitRepo: String
    var gitRepoName: String
    var themeIndex: Int
    var dateFormat: String
    var layout: String
    var paneSizes: [Double]
    var syncInterval: Int
    var setupDone: Bool
    var keybindings: [String: String]

    enum CodingKeys: String, CodingKey {
        case storageMode = "storage_mode"
        case localPath = "local_path"
        case gitRepo = "git_repo"
        case gitRepoName = "git_repo_name"
        case themeIndex = "theme_index"
        case dateFormat = "date_format"
        case layout
        case paneSizes = "pane_sizes"
        case syncInterval = "sync_interval"
        case setupDone = "setup_done"
        case keybindings
    }

    static let `default` = AppSettings(
        storageMode: "local",
        localPath: "",
        gitRepo: "",
        gitRepoName: "tally-md-log",
        themeIndex: 0,
        dateFormat: "%Y-%m-%d",
        layout: "vertical",
        paneSizes: [33, 33, 34],
        syncInterval: 5,
        setupDone: false,
        keybindings: [:]
    )
}
