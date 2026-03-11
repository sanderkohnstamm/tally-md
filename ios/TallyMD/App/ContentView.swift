import SwiftUI

enum PaneType: String, CaseIterable, Identifiable {
    case todo, today, done
    var id: String { rawValue }

    var label: String {
        switch self {
        case .todo: return "Todo"
        case .today: return "Today"
        case .done: return "Done"
        }
    }
}

struct ContentView: View {
    @EnvironmentObject var vm: AppViewModel
    @State private var showSettings = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // Status bar
                StatusBarView()

                // Accordion panes
                ScrollView {
                    VStack(spacing: 0) {
                        ForEach(PaneType.allCases) { pane in
                            PaneView(pane: pane)
                        }
                    }
                }

                // Action bar
                ActionBarView()
            }
            .background(vm.theme.bg)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    Text("Tally.md")
                        .font(.headline)
                        .foregroundColor(vm.theme.text)
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        showSettings = true
                    } label: {
                        Image(systemName: "gearshape")
                            .foregroundColor(vm.theme.subtext)
                    }
                }
            }
            .toolbarBackground(vm.theme.surface, for: .navigationBar)
            .toolbarBackground(.visible, for: .navigationBar)
            .sheet(isPresented: $showSettings) {
                SettingsView()
                    .environmentObject(vm)
            }
            .sheet(isPresented: $vm.showFirstTimeSetup) {
                SettingsView(isFirstTime: true)
                    .environmentObject(vm)
                    .interactiveDismissDisabled()
            }
        }
    }
}

struct StatusBarView: View {
    @EnvironmentObject var vm: AppViewModel

    var syncColor: Color {
        switch vm.syncState {
        case .ok: return vm.theme.green
        case .dirty: return vm.theme.yellow
        case .error: return vm.theme.red
        case .active: return vm.theme.blue
        }
    }

    var body: some View {
        HStack {
            if vm.settings.storageMode == "git" {
                HStack(spacing: 4) {
                    Circle()
                        .fill(syncColor)
                        .frame(width: 8, height: 8)
                    Text("git: \(vm.settings.gitRepoName)")
                        .font(.caption)
                        .foregroundColor(vm.theme.subtext)
                }
            } else {
                Text("local")
                    .font(.caption)
                    .foregroundColor(vm.theme.subtext)
            }

            Spacer()

            if !vm.statusMessage.isEmpty {
                Text(vm.statusMessage)
                    .font(.caption)
                    .foregroundColor(vm.theme.green)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 6)
        .background(vm.theme.statusBg)
    }
}
