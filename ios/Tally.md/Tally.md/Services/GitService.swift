import Foundation

/// Git sync via GitHub REST API.
/// Uses the Contents API for simple file operations on the 4-file repo.
actor GitService {
    private let session = URLSession.shared

    struct FileInfo {
        let content: String
        let sha: String
    }

    // MARK: - Public API

    /// Clone/init: ensure remote repo has the default files. Fetch them locally.
    func initRepo(settings: AppSettings, token: String) async throws -> String {
        let dir = FileService.todosDirectory(settings: settings)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        for name in ["todo.md", "today.md", "done.md", "settings.json"] {
            let remote = try? await fetchFile(name: name, settings: settings, token: token)
            if remote == nil {
                let localContent = readLocal(name: name, dir: dir)
                try await pushFile(
                    name: name, content: localContent, sha: nil,
                    message: "Initial commit from Tally.md",
                    settings: settings, token: token
                )
            }
        }

        _ = try await pull(settings: settings, token: token)
        return "Repo initialized"
    }

    /// Pull: fetch all files from remote and write locally.
    func pull(settings: AppSettings, token: String) async throws -> String {
        let dir = FileService.todosDirectory(settings: settings)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)

        var pulled = 0
        for name in ["todo.md", "today.md", "done.md"] {
            if let file = try? await fetchFile(name: name, settings: settings, token: token) {
                let path = dir.appendingPathComponent(name)
                try? file.content.write(to: path, atomically: true, encoding: .utf8)
                pulled += 1
            }
        }

        if let file = try? await fetchFile(name: "settings.json", settings: settings, token: token) {
            let path = dir.appendingPathComponent("settings.json")
            try? file.content.write(to: path, atomically: true, encoding: .utf8)
        }

        return pulled > 0 ? "Pulled \(pulled) files" : "Nothing to pull"
    }

    /// Push: upload local files to remote (only changed ones).
    func push(settings: AppSettings, token: String) async throws -> String {
        let dir = FileService.todosDirectory(settings: settings)
        var pushed = 0

        for name in ["todo.md", "today.md", "done.md", "settings.json"] {
            let localContent = readLocal(name: name, dir: dir)

            // Get current SHA from remote
            let remote = try? await fetchFile(name: name, settings: settings, token: token)
            let remoteSha = remote?.sha
            let remoteContent = remote?.content ?? ""

            // Only push if content changed
            if localContent != remoteContent {
                let timestamp = formatTimestamp()
                try await pushFile(
                    name: name, content: localContent, sha: remoteSha,
                    message: "Tally.md sync \(timestamp)",
                    settings: settings, token: token
                )
                pushed += 1
            }
        }

        return pushed > 0 ? "Pushed \(pushed) files" : "Nothing to push"
    }

    // MARK: - GitHub API

    private func parseRepo(url: String) -> (owner: String, repo: String)? {
        let cleaned = url
            .replacingOccurrences(of: "https://github.com/", with: "")
            .replacingOccurrences(of: ".git", with: "")
            .trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let parts = cleaned.components(separatedBy: "/")
        guard parts.count >= 2 else { return nil }
        return (parts[0], parts[1])
    }

    private func fetchFile(
        name: String, settings: AppSettings, token: String
    ) async throws -> FileInfo {
        guard let repo = parseRepo(url: settings.gitRepo) else {
            throw GitError.invalidRepoURL
        }

        let urlString = "https://api.github.com/repos/\(repo.owner)/\(repo.repo)/contents/\(name)"
        guard let url = URL(string: urlString) else {
            throw GitError.invalidRepoURL
        }

        var request = URLRequest(url: url)
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/vnd.github.v3+json", forHTTPHeaderField: "Accept")
        request.cachePolicy = .reloadIgnoringLocalCacheData

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw GitError.networkError("Invalid response")
        }

        if httpResponse.statusCode == 404 {
            throw GitError.fileNotFound(name)
        }

        guard httpResponse.statusCode == 200 else {
            throw GitError.networkError("HTTP \(httpResponse.statusCode)")
        }

        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let contentB64 = json["content"] as? String,
              let sha = json["sha"] as? String
        else {
            throw GitError.parseError
        }

        let cleanB64 = contentB64.replacingOccurrences(of: "\n", with: "")
        guard let contentData = Data(base64Encoded: cleanB64),
              let content = String(data: contentData, encoding: .utf8)
        else {
            throw GitError.parseError
        }

        return FileInfo(content: content, sha: sha)
    }

    /// Push a file to the GitHub Contents API (create or update).
    private func pushFile(
        name: String, content: String, sha: String?,
        message: String, settings: AppSettings, token: String
    ) async throws {
        guard let repo = parseRepo(url: settings.gitRepo) else {
            throw GitError.invalidRepoURL
        }

        let urlString = "https://api.github.com/repos/\(repo.owner)/\(repo.repo)/contents/\(name)"
        guard let url = URL(string: urlString) else {
            throw GitError.invalidRepoURL
        }

        let contentData = content.data(using: .utf8) ?? Data()
        let base64Content = contentData.base64EncodedString()

        var body: [String: Any] = [
            "message": message,
            "content": base64Content,
        ]
        if let sha = sha {
            body["sha"] = sha
        }

        var request = URLRequest(url: url)
        request.httpMethod = "PUT"
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        request.setValue("application/vnd.github.v3+json", forHTTPHeaderField: "Accept")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (_, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse,
              (200...201).contains(httpResponse.statusCode)
        else {
            let code = (response as? HTTPURLResponse)?.statusCode ?? 0
            throw GitError.networkError("Push failed: HTTP \(code)")
        }
    }

    // MARK: - Helpers

    private func readLocal(name: String, dir: URL) -> String {
        let path = dir.appendingPathComponent(name)
        return (try? String(contentsOf: path, encoding: .utf8)) ?? ""
    }

    private func formatTimestamp() -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm"
        return formatter.string(from: Date())
    }
}

enum GitError: LocalizedError {
    case invalidRepoURL
    case fileNotFound(String)
    case networkError(String)
    case parseError

    var errorDescription: String? {
        switch self {
        case .invalidRepoURL: return "Invalid repository URL"
        case .fileNotFound(let name): return "File not found: \(name)"
        case .networkError(let msg): return msg
        case .parseError: return "Failed to parse API response"
        }
    }
}
