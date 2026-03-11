import SwiftUI

struct ThemePalette {
    let name: String
    let bg: Color
    let surface: Color
    let overlay: Color
    let text: Color
    let subtext: Color
    let blue: Color
    let green: Color
    let mauve: Color
    let red: Color
    let yellow: Color
    let teal: Color
    let border: Color
    let statusBg: Color
}

extension Color {
    init(hex: String) {
        let hex = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        var int: UInt64 = 0
        Scanner(string: hex).scanHexInt64(&int)
        let r = Double((int >> 16) & 0xFF) / 255
        let g = Double((int >> 8) & 0xFF) / 255
        let b = Double(int & 0xFF) / 255
        self.init(red: r, green: g, blue: b)
    }
}

let palettes: [ThemePalette] = [
    ThemePalette(
        name: "White on Black",
        bg: Color(hex: "#000000"), surface: Color(hex: "#0a0a0a"),
        overlay: Color(hex: "#1a1a1a"), text: Color(hex: "#ffffff"),
        subtext: Color(hex: "#999999"), blue: Color(hex: "#ffffff"),
        green: Color(hex: "#ffffff"), mauve: Color(hex: "#ffffff"),
        red: Color(hex: "#ffffff"), yellow: Color(hex: "#ffffff"),
        teal: Color(hex: "#ffffff"), border: Color(hex: "#333333"),
        statusBg: Color(hex: "#000000")
    ),
    ThemePalette(
        name: "Black on White",
        bg: Color(hex: "#ffffff"), surface: Color(hex: "#f5f5f5"),
        overlay: Color(hex: "#e8e8e8"), text: Color(hex: "#000000"),
        subtext: Color(hex: "#666666"), blue: Color(hex: "#000000"),
        green: Color(hex: "#000000"), mauve: Color(hex: "#000000"),
        red: Color(hex: "#000000"), yellow: Color(hex: "#000000"),
        teal: Color(hex: "#000000"), border: Color(hex: "#cccccc"),
        statusBg: Color(hex: "#f0f0f0")
    ),
    ThemePalette(
        name: "Catppuccin",
        bg: Color(hex: "#1e1e2e"), surface: Color(hex: "#181825"),
        overlay: Color(hex: "#313244"), text: Color(hex: "#cdd6f4"),
        subtext: Color(hex: "#a6adc8"), blue: Color(hex: "#89b4fa"),
        green: Color(hex: "#a6e3a1"), mauve: Color(hex: "#cba6f7"),
        red: Color(hex: "#f38ba8"), yellow: Color(hex: "#f9e2af"),
        teal: Color(hex: "#94e2d5"), border: Color(hex: "#45475a"),
        statusBg: Color(hex: "#11111b")
    ),
    ThemePalette(
        name: "Rose Pine",
        bg: Color(hex: "#191724"), surface: Color(hex: "#1f1d2e"),
        overlay: Color(hex: "#26233a"), text: Color(hex: "#e0def4"),
        subtext: Color(hex: "#908caa"), blue: Color(hex: "#9ccfd8"),
        green: Color(hex: "#31748f"), mauve: Color(hex: "#c4a7e7"),
        red: Color(hex: "#eb6f92"), yellow: Color(hex: "#f6c177"),
        teal: Color(hex: "#9ccfd8"), border: Color(hex: "#2a2837"),
        statusBg: Color(hex: "#16141f")
    ),
    ThemePalette(
        name: "Tokyo Night",
        bg: Color(hex: "#1a1b26"), surface: Color(hex: "#16161e"),
        overlay: Color(hex: "#292e42"), text: Color(hex: "#c0caf5"),
        subtext: Color(hex: "#787c99"), blue: Color(hex: "#7aa2f7"),
        green: Color(hex: "#9ece6a"), mauve: Color(hex: "#bb9af7"),
        red: Color(hex: "#f7768e"), yellow: Color(hex: "#e0af68"),
        teal: Color(hex: "#73daca"), border: Color(hex: "#3b4261"),
        statusBg: Color(hex: "#13131e")
    ),
    ThemePalette(
        name: "Soft Ember",
        bg: Color(hex: "#1c1917"), surface: Color(hex: "#181412"),
        overlay: Color(hex: "#2c2622"), text: Color(hex: "#e7ddd5"),
        subtext: Color(hex: "#a8998e"), blue: Color(hex: "#e8a87c"),
        green: Color(hex: "#a3be8c"), mauve: Color(hex: "#d4a0c0"),
        red: Color(hex: "#cf8989"), yellow: Color(hex: "#e8c47c"),
        teal: Color(hex: "#8fbcbb"), border: Color(hex: "#3d3530"),
        statusBg: Color(hex: "#141110")
    ),
    ThemePalette(
        name: "Nord",
        bg: Color(hex: "#2e3440"), surface: Color(hex: "#272c36"),
        overlay: Color(hex: "#3b4252"), text: Color(hex: "#d8dee9"),
        subtext: Color(hex: "#939aad"), blue: Color(hex: "#88c0d0"),
        green: Color(hex: "#a3be8c"), mauve: Color(hex: "#b48ead"),
        red: Color(hex: "#bf616a"), yellow: Color(hex: "#ebcb8b"),
        teal: Color(hex: "#8fbcbb"), border: Color(hex: "#4c566a"),
        statusBg: Color(hex: "#242933")
    ),
    ThemePalette(
        name: "Moonlight",
        bg: Color(hex: "#1e2030"), surface: Color(hex: "#191b28"),
        overlay: Color(hex: "#2f334d"), text: Color(hex: "#c8d3f5"),
        subtext: Color(hex: "#828bb8"), blue: Color(hex: "#82aaff"),
        green: Color(hex: "#c3e88d"), mauve: Color(hex: "#c099ff"),
        red: Color(hex: "#ff757f"), yellow: Color(hex: "#ffc777"),
        teal: Color(hex: "#86e1fc"), border: Color(hex: "#3b3f5c"),
        statusBg: Color(hex: "#161825")
    ),
]
