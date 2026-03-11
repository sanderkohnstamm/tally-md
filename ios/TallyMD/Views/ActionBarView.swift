import SwiftUI

struct ActionBarView: View {
    @EnvironmentObject var vm: AppViewModel

    private var canMoveForward: Bool {
        vm.expandedPane == .todo || vm.expandedPane == .today
    }

    private var canMoveBack: Bool {
        vm.expandedPane == .today || vm.expandedPane == .done
    }

    private var forwardLabel: String {
        switch vm.expandedPane {
        case .todo: return "todo -> today"
        case .today: return "today -> done"
        default: return "forward"
        }
    }

    private var backLabel: String {
        switch vm.expandedPane {
        case .today: return "today -> todo"
        case .done: return "done -> todo"
        default: return "back"
        }
    }

    var body: some View {
        HStack(spacing: 12) {
            // Move back
            Button {
                vm.moveBack()
            } label: {
                Label(backLabel, systemImage: "arrow.left.circle.fill")
                    .font(.caption)
            }
            .disabled(!canMoveBack)

            Spacer()

            // Undo
            Button {
                vm.undoManagers[vm.expandedPane ?? .todo]?.undo()
            } label: {
                Image(systemName: "arrow.uturn.backward")
            }
            .disabled(vm.expandedPane == nil)

            // Redo
            Button {
                vm.undoManagers[vm.expandedPane ?? .todo]?.redo()
            } label: {
                Image(systemName: "arrow.uturn.forward")
            }
            .disabled(vm.expandedPane == nil)

            // Sync (git mode only)
            if vm.settings.storageMode == "git" {
                Button {
                    Task { await vm.syncFull() }
                } label: {
                    Image(systemName: "arrow.triangle.2.circlepath")
                }
                .disabled(vm.syncState == .active)
            }

            Spacer()

            // Move forward
            Button {
                vm.moveForward()
            } label: {
                Label(forwardLabel, systemImage: "arrow.right.circle.fill")
                    .font(.caption)
            }
            .disabled(!canMoveForward)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(vm.theme.surface)
        .foregroundColor(vm.theme.text)
        .tint(vm.theme.blue)
    }
}
