import SwiftUI

struct ActionBarView: View {
    @EnvironmentObject var vm: AppViewModel

    private var backLabel: String {
        switch vm.expandedPane {
        case .today: return "todo"
        case .done: return "today"
        default: return ""
        }
    }

    private var canMoveBack: Bool {
        vm.expandedPane == .today || vm.expandedPane == .done
    }

    private var forwardLabel: String {
        switch vm.expandedPane {
        case .todo: return "today"
        case .today: return "done"
        case .done: return "todo"
        default: return ""
        }
    }

    var body: some View {
        HStack {
            // Move back
            Button {
                vm.moveBack()
            } label: {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.left.circle.fill")
                    Text(backLabel)
                }
                .font(.caption)
            }
            .opacity(canMoveBack ? 1 : 0)
            .disabled(!canMoveBack)

            Spacer()

            // Pull / Push (git mode only)
            if vm.settings.storageMode == "git" {
                HStack(spacing: 20) {
                    Button {
                        Task { await vm.pullOnly() }
                    } label: {
                        Image(systemName: "arrow.down.circle")
                    }
                    .disabled(vm.syncState == .active)

                    Button {
                        Task { await vm.pushOnly() }
                    } label: {
                        Image(systemName: "arrow.up.circle")
                    }
                    .disabled(vm.syncState == .active)
                }
            }

            Spacer()

            // Move forward (cycles: todo → today → done → todo)
            Button {
                vm.moveForward()
            } label: {
                HStack(spacing: 4) {
                    Text(forwardLabel)
                    Image(systemName: "arrow.right.circle.fill")
                }
                .font(.caption)
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(vm.theme.surface)
        .foregroundColor(vm.theme.text)
        .tint(vm.theme.blue)
    }
}
