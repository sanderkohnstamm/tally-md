import SwiftUI
import Combine

enum SyncState {
    case ok, dirty, error, active
}

@MainActor
final class AppViewModel: ObservableObject {
    // MARK: - Published State

    @Published var todoContent = ""
    @Published var todayContent = ""
    @Published var doneContent = ""

    @Published var expandedPane: PaneType? = .todo
    @Published var cursorLine: Int = 0

    @Published var settings: AppSettings = .default
    @Published var syncState: SyncState = .ok
    @Published var statusMessage = ""

    @Published var showFirstTimeSetup = false
    @Published var isDirty: [PaneType: Bool] = [.todo: false, .today: false, .done: false]

    // MARK: - Theme

    var theme: ThemePalette {
        let idx = min(settings.themeIndex, palettes.count - 1)
        return palettes[max(0, idx)]
    }

    // MARK: - Private

    private let autoSaveDebouncer = Debouncer(delay: 1.0)
    private let gitService = GitService()
    private var syncTimer: Timer?
    private var statusTimer: Timer?

    // Undo managers per pane
    let undoManagers: [PaneType: UndoManager] = [
        .todo: UndoManager(),
        .today: UndoManager(),
        .done: UndoManager(),
    ]

    // MARK: - Lifecycle

    func launch() {
        settings = FileService.loadSettings()

        if settings.setupDone {
            loadFiles()
            if settings.storageMode == "git" && !settings.gitRepo.isEmpty {
                Task { await pullSilent() }
            }
            startSyncTimer()
        } else {
            FileService.seedWelcomeContentIfNeeded(settings: settings)
            loadFiles()
            showFirstTimeSetup = true
        }
    }

    // MARK: - File Operations

    func loadFiles() {
        let files = FileService.loadFiles(settings: settings)
        todoContent = files.todo
        todayContent = files.today

        // Fill empty days in done.md
        let filled = DateHeaderEngine.fillEmptyDays(
            text: files.done, today: Date(), dateFormat: settings.dateFormat
        )
        doneContent = filled
        isDirty = [.todo: false, .today: false, .done: false]
    }

    func saveFiles() {
        FileService.saveFiles(
            todo: todoContent, today: todayContent, done: doneContent,
            settings: settings
        )
        isDirty = [.todo: false, .today: false, .done: false]
    }

    func scheduleAutoSave() {
        autoSaveDebouncer.call { [weak self] in
            self?.saveFiles()
        }
    }

    func markDirty(pane: PaneType) {
        isDirty[pane] = true
        if settings.storageMode == "git" && syncState != .error {
            syncState = .dirty
        }
    }

    // MARK: - Content Binding

    func content(for pane: PaneType) -> Binding<String> {
        switch pane {
        case .todo:
            return Binding(
                get: { self.todoContent },
                set: { self.todoContent = $0; self.markDirty(pane: .todo); self.scheduleAutoSave() }
            )
        case .today:
            return Binding(
                get: { self.todayContent },
                set: { self.todayContent = $0; self.markDirty(pane: .today); self.scheduleAutoSave() }
            )
        case .done:
            return Binding(
                get: { self.doneContent },
                set: { self.doneContent = $0; self.markDirty(pane: .done); self.scheduleAutoSave() }
            )
        }
    }

    // MARK: - Item Movement

    func moveForward() {
        guard let pane = expandedPane else { return }

        switch pane {
        case .todo:
            // todo -> today
            if let result = ItemMovementEngine.moveItemForward(
                source: todoContent, target: todayContent, cursorLine: cursorLine
            ) {
                todoContent = result.0
                todayContent = result.1
                saveFiles()
                showStatus("todo -> today")
            }
        case .today:
            // today -> done
            if let result = ItemMovementEngine.completeItem(
                source: todayContent, target: doneContent, cursorLine: cursorLine,
                today: Date(), dateFormat: settings.dateFormat
            ) {
                todayContent = result.0
                doneContent = result.1
                saveFiles()
                showStatus("today -> done")
            }
        case .done:
            // done -> todo (cycle back)
            if let result = ItemMovementEngine.recoverItem(
                source: doneContent, target: todoContent, cursorLine: cursorLine
            ) {
                doneContent = result.0
                todoContent = result.1
                saveFiles()
                showStatus("done → todo")
            }
        }
    }

    func moveBack() {
        guard let pane = expandedPane else { return }

        switch pane {
        case .todo:
            break
        case .today:
            // today -> todo
            if let result = ItemMovementEngine.moveItemBack(
                source: todayContent, target: todoContent, cursorLine: cursorLine
            ) {
                todayContent = result.0
                todoContent = result.1
                saveFiles()
                showStatus("today -> todo")
            }
        case .done:
            // done -> todo
            if let result = ItemMovementEngine.recoverItem(
                source: doneContent, target: todoContent, cursorLine: cursorLine
            ) {
                doneContent = result.0
                todoContent = result.1
                saveFiles()
                showStatus("done -> todo")
            }
        }
    }

    // MARK: - Git Sync

    func syncFull() async {
        guard settings.storageMode == "git",
              !settings.gitRepo.isEmpty,
              let token = KeychainService.getToken()
        else { return }

        syncState = .active
        saveFiles()

        // Push local edits first
        do {
            showStatus("Pushing...")
            let pushResult = try await gitService.push(settings: settings, token: token)
            showStatus(pushResult)
        } catch {
            showStatus("Push error: \(error.localizedDescription)")
        }

        // Then pull remote changes
        do {
            showStatus("Pulling...")
            let pullResult = try await gitService.pull(settings: settings, token: token)
            loadFiles()
            syncState = .ok
            showStatus(pullResult)
        } catch {
            syncState = .error
            showStatus("Pull error: \(error.localizedDescription)")
        }
    }

    func pullOnly() async {
        guard settings.storageMode == "git",
              !settings.gitRepo.isEmpty,
              let token = KeychainService.getToken()
        else { return }

        syncState = .active
        showStatus("Pulling...")
        do {
            let result = try await gitService.pull(settings: settings, token: token)
            loadFiles()
            syncState = .ok
            showStatus(result)
        } catch {
            syncState = .error
            showStatus("Pull error: \(error.localizedDescription)")
        }
    }

    func pushOnly() async {
        guard settings.storageMode == "git",
              !settings.gitRepo.isEmpty,
              let token = KeychainService.getToken()
        else { return }

        syncState = .active
        saveFiles()
        showStatus("Pushing...")
        do {
            let result = try await gitService.push(settings: settings, token: token)
            syncState = .ok
            showStatus(result)
        } catch {
            syncState = .error
            showStatus("Push error: \(error.localizedDescription)")
        }
    }

    func initRepo() async throws -> String {
        guard let token = KeychainService.getToken() else {
            throw GitError.networkError("No token stored")
        }
        saveFiles()
        let result = try await gitService.initRepo(settings: settings, token: token)
        loadFiles()
        return result
    }

    func pushIfNeeded() {
        guard settings.storageMode == "git",
              !settings.gitRepo.isEmpty
        else { return }

        Task {
            await syncFull()
        }
    }

    private func pullSilent() async {
        guard let token = KeychainService.getToken() else { return }
        syncState = .active
        do {
            _ = try await gitService.pull(settings: settings, token: token)
            loadFiles()
            syncState = .ok
        } catch {
            syncState = .error
        }
    }

    // MARK: - Sync Timer

    func startSyncTimer() {
        stopSyncTimer()
        guard settings.storageMode == "git",
              settings.syncInterval > 0
        else { return }

        let interval = TimeInterval(settings.syncInterval * 60)
        syncTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: true) { [weak self] _ in
            Task { @MainActor in
                await self?.syncFull()
            }
        }
    }

    func stopSyncTimer() {
        syncTimer?.invalidate()
        syncTimer = nil
    }

    // MARK: - Settings

    func saveSettings() {
        FileService.saveSettings(settings)
        startSyncTimer()
    }

    func reformatDoneDates(newFormat: String) {
        doneContent = DateHeaderEngine.reformatDateHeaders(text: doneContent, newFormat: newFormat)
    }

    // MARK: - Status

    func showStatus(_ message: String) {
        statusMessage = message
        statusTimer?.invalidate()
        statusTimer = Timer.scheduledTimer(withTimeInterval: 3, repeats: false) { _ in
            Task { @MainActor [weak self] in
                self?.statusMessage = ""
            }
        }
    }
}
