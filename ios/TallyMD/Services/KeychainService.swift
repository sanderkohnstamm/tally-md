import Foundation
import KeychainAccess

/// Stores and retrieves the GitHub PAT from the iOS Keychain.
enum KeychainService {
    private static let keychain = Keychain(service: "com.sanderkohnstamm.tallymd")
    private static let tokenKey = "git-token"

    static func storeToken(_ token: String) throws {
        try keychain.set(token, key: tokenKey)
    }

    static func getToken() -> String? {
        try? keychain.get(tokenKey)
    }

    static func hasToken() -> Bool {
        getToken() != nil
    }

    static func deleteToken() throws {
        try keychain.remove(tokenKey)
    }
}
