import SwiftUI
import UIKit

/// A UITextView wrapper that exposes cursor line position and supports undo/redo.
struct MarkdownTextView: UIViewRepresentable {
    @Binding var text: String
    @Binding var cursorLine: Int
    let theme: ThemePalette
    let undoManager: UndoManager?

    func makeCoordinator() -> Coordinator {
        Coordinator(self)
    }

    func makeUIView(context: Context) -> UITextView {
        let textView = UITextView()
        textView.delegate = context.coordinator
        textView.isEditable = true
        textView.isScrollEnabled = true
        textView.font = .monospacedSystemFont(ofSize: 15, weight: .regular)
        textView.autocapitalizationType = .none
        textView.autocorrectionType = .no
        textView.smartDashesType = .no
        textView.smartQuotesType = .no
        textView.smartInsertDeleteType = .no
        textView.keyboardDismissMode = .interactive
        textView.textContainerInset = UIEdgeInsets(top: 12, left: 8, bottom: 12, right: 8)
        textView.alwaysBounceVertical = true

        // Use the provided undo manager
        if let um = undoManager {
            textView.undoManager?.removeAllActions()
            // UITextView manages its own undo stack; we expose it through the coordinator
            context.coordinator.externalUndoManager = um
        }

        applyTheme(textView)
        return textView
    }

    func updateUIView(_ textView: UITextView, context: Context) {
        if textView.text != text {
            let selectedRange = textView.selectedRange
            textView.text = text
            // Restore cursor if within bounds
            if selectedRange.location <= text.count {
                textView.selectedRange = selectedRange
            }
        }
        applyTheme(textView)
    }

    private func applyTheme(_ textView: UITextView) {
        textView.backgroundColor = UIColor(theme.bg)
        textView.textColor = UIColor(theme.text)
        textView.tintColor = UIColor(theme.blue)
    }

    class Coordinator: NSObject, UITextViewDelegate {
        var parent: MarkdownTextView
        var externalUndoManager: UndoManager?

        init(_ parent: MarkdownTextView) {
            self.parent = parent
        }

        func textViewDidChange(_ textView: UITextView) {
            parent.text = textView.text
        }

        func textViewDidChangeSelection(_ textView: UITextView) {
            let cursorPos = textView.selectedRange.location
            let text = textView.text ?? ""
            let prefix = String(text.prefix(cursorPos))
            let line = prefix.components(separatedBy: "\n").count - 1
            parent.cursorLine = line
        }
    }
}
