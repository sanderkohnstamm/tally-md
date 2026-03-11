import SwiftUI

@main
struct TallyMDApp: App {
    @StateObject private var appVM = AppViewModel()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appVM)
                .onAppear {
                    appVM.launch()
                }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .background {
                        appVM.saveFiles()
                        appVM.pushIfNeeded()
                    }
                }
        }
    }
}
