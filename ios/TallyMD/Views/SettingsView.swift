import SwiftUI

struct SettingsView: View {
    var isFirstTime = false

    @EnvironmentObject var vm: AppViewModel
    @Environment(\.dismiss) private var dismiss

    @State private var storageMode: String = "local"
    @State private var gitRepo: String = ""
    @State private var gitRepoName: String = "tally-md-log"
    @State private var gitToken: String = ""
    @State private var syncInterval: Int = 5
    @State private var themeIndex: Int = 0
    @State private var dateFormat: String = "%Y-%m-%d"

    @State private var initStatus: String = ""
    @State private var isInitializing = false

    var body: some View {
        NavigationStack {
            Form {
                // Storage mode
                Section("Storage") {
                    Picker("Mode", selection: $storageMode) {
                        Text("Local").tag("local")
                        Text("Git").tag("git")
                    }
                    .pickerStyle(.segmented)
                }

                // Git settings
                if storageMode == "git" {
                    Section("Git Repository") {
                        TextField("Repo name", text: $gitRepoName)
                            .autocapitalization(.none)
                            .disableAutocorrection(true)

                        TextField("Repo URL", text: $gitRepo)
                            .autocapitalization(.none)
                            .disableAutocorrection(true)
                            .keyboardType(.URL)

                        SecureField("Personal Access Token", text: $gitToken)
                            .autocapitalization(.none)

                        if KeychainService.hasToken() && gitToken.isEmpty {
                            Text("Token stored in keychain")
                                .font(.caption)
                                .foregroundColor(vm.theme.green)
                        }

                        HStack {
                            Button("Initialize Repo") {
                                Task { await initializeRepo() }
                            }
                            .disabled(isInitializing || gitRepo.isEmpty)

                            if !initStatus.isEmpty {
                                Text(initStatus)
                                    .font(.caption)
                                    .foregroundColor(vm.theme.subtext)
                            }
                        }
                    }

                    Section("Auto-Sync") {
                        Picker("Interval", selection: $syncInterval) {
                            Text("Off").tag(0)
                            Text("5 min").tag(5)
                            Text("15 min").tag(15)
                            Text("30 min").tag(30)
                        }
                        .pickerStyle(.segmented)
                    }
                }

                // Theme
                Section("Theme") {
                    ThemePickerView(selectedIndex: $themeIndex)
                }

                // Date format
                Section("Date Format") {
                    DateFormatPickerView(selectedFormat: $dateFormat)
                }
            }
            .navigationTitle(isFirstTime ? "Welcome to Tally.md" : "Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if !isFirstTime {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { dismiss() }
                    }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { save() }
                }
            }
            .onAppear { loadCurrent() }
            .onChange(of: themeIndex) { _, idx in
                vm.settings.themeIndex = idx
            }
            .onChange(of: dateFormat) { oldFormat, newFormat in
                if oldFormat != newFormat {
                    vm.reformatDoneDates(newFormat: newFormat)
                }
            }
        }
    }

    private func loadCurrent() {
        storageMode = vm.settings.storageMode
        gitRepo = vm.settings.gitRepo
        gitRepoName = vm.settings.gitRepoName
        syncInterval = vm.settings.syncInterval
        themeIndex = vm.settings.themeIndex
        dateFormat = vm.settings.dateFormat
    }

    private func save() {
        // Store token if provided
        if !gitToken.isEmpty {
            try? KeychainService.storeToken(gitToken)
        }

        vm.settings.storageMode = storageMode
        vm.settings.gitRepo = gitRepo
        vm.settings.gitRepoName = gitRepoName
        vm.settings.syncInterval = syncInterval
        vm.settings.themeIndex = themeIndex
        vm.settings.dateFormat = dateFormat
        vm.settings.setupDone = true

        vm.saveSettings()
        vm.loadFiles()

        // Pull on first setup with git
        if storageMode == "git" && !gitRepo.isEmpty {
            Task { await vm.syncFull() }
        }

        dismiss()
    }

    private func initializeRepo() async {
        // Store token first
        if !gitToken.isEmpty {
            try? KeychainService.storeToken(gitToken)
        }

        // Update settings temporarily for init
        vm.settings.storageMode = "git"
        vm.settings.gitRepo = gitRepo
        vm.settings.gitRepoName = gitRepoName

        isInitializing = true
        initStatus = "Initializing..."
        do {
            let result = try await vm.initRepo()
            initStatus = result
        } catch {
            initStatus = "Error: \(error.localizedDescription)"
        }
        isInitializing = false
    }
}

// MARK: - Theme Picker

struct ThemePickerView: View {
    @Binding var selectedIndex: Int

    let columns = Array(repeating: GridItem(.flexible(), spacing: 12), count: 4)

    var body: some View {
        LazyVGrid(columns: columns, spacing: 12) {
            ForEach(0..<palettes.count, id: \.self) { i in
                let p = palettes[i]
                VStack(spacing: 4) {
                    Circle()
                        .fill(p.bg)
                        .frame(width: 36, height: 36)
                        .overlay(
                            Circle()
                                .strokeBorder(
                                    i == selectedIndex ? p.text : p.border,
                                    lineWidth: i == selectedIndex ? 3 : 1
                                )
                        )
                    Text(p.name)
                        .font(.system(size: 9))
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                }
                .onTapGesture {
                    selectedIndex = i
                }
            }
        }
        .padding(.vertical, 4)
    }
}

// MARK: - Date Format Picker

struct DateFormatPickerView: View {
    @Binding var selectedFormat: String

    var body: some View {
        ForEach(DateParsing.allFormats, id: \.self) { fmt in
            Button {
                selectedFormat = fmt
            } label: {
                HStack {
                    Text(DateParsing.preview(strftimeFormat: fmt))
                        .foregroundColor(.primary)
                    Spacer()
                    if fmt == selectedFormat {
                        Image(systemName: "checkmark")
                            .foregroundColor(.accentColor)
                    }
                }
            }
        }
    }
}
