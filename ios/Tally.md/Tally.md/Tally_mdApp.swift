import SwiftUI

@main
struct Tally_mdApp: App {
    @StateObject private var appVM = AppViewModel()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appVM)
                .onAppear {
                    appVM.launch()
                    updateWindowBackground()
                }
                .onChange(of: appVM.settings.themeIndex) { _, _ in
                    updateWindowBackground()
                }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .background {
                        appVM.saveFiles()
                        appVM.pushIfNeeded()
                    }
                }
        }
    }

    private func updateWindowBackground() {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first
        else { return }
        let surfaceColor = UIColor(appVM.theme.surface)
        window.backgroundColor = surfaceColor
        window.rootViewController?.view.backgroundColor = surfaceColor
    }
}
