import SwiftUI

struct PaneView: View {
    let pane: PaneType
    @EnvironmentObject var vm: AppViewModel

    private var isExpanded: Bool {
        vm.expandedPane == pane
    }

    private var isDirty: Bool {
        vm.isDirty[pane] ?? false
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header bar — tap to expand/collapse
            Button {
                withAnimation(.easeInOut(duration: 0.25)) {
                    vm.expandedPane = isExpanded ? nil : pane
                }
            } label: {
                HStack {
                    Text(pane.label)
                        .font(.system(.subheadline, weight: .semibold))
                        .foregroundColor(isExpanded ? vm.theme.blue : vm.theme.subtext)

                    if isDirty {
                        Text("[+]")
                            .font(.caption)
                            .foregroundColor(vm.theme.yellow)
                    }

                    Spacer()

                    Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        .font(.caption)
                        .foregroundColor(vm.theme.subtext)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(isExpanded ? vm.theme.overlay : vm.theme.surface)
            }

            // Divider
            Rectangle()
                .fill(vm.theme.border)
                .frame(height: 1)

            // Editor (only when expanded)
            if isExpanded {
                MarkdownTextView(
                    text: vm.content(for: pane),
                    cursorLine: $vm.cursorLine,
                    theme: vm.theme,
                    undoManager: vm.undoManagers[pane]
                )
                .frame(minHeight: 200, maxHeight: .infinity)
            }
        }
    }
}
