import Foundation

/// Simple debouncer that fires an action after a delay, resetting on each call.
final class Debouncer {
    private var timer: Timer?
    private let delay: TimeInterval

    init(delay: TimeInterval = 1.0) {
        self.delay = delay
    }

    func call(action: @escaping () -> Void) {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { _ in
            action()
        }
    }

    func cancel() {
        timer?.invalidate()
        timer = nil
    }
}
