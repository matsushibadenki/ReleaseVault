// The OS owns the passkey. Only a PRF-wrapped copy of the random vault key goes to SQLite.
import AppKit
import AuthenticationServices
import CryptoKit
import Foundation
import Security

private func relyingParty() -> String? {
    guard let task = SecTaskCreateFromSelf(nil),
          let entries = SecTaskCopyValueForEntitlement(task, "com.apple.developer.associated-domains" as CFString, nil) as? [String] else { return nil }
    let prefix = "webcredentials:"
    let domains = entries.filter { $0.hasPrefix(prefix) }.map { String($0.dropFirst(prefix.count)) }
    guard domains.count == 1, let domain = domains.first,
          domain.range(of: #"^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$"#, options: .regularExpression) != nil else { return nil }
    return domain
}

private struct PasskeyRecord: Codable {
    let rp: String
    let id, salt, sealed: Data
}
private enum PasskeyError: Error { case failed }
private func randomBytes() throws -> Data {
    var bytes = Data(count: 32)
    guard bytes.withUnsafeMutableBytes({ SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }) == errSecSuccess else { throw PasskeyError.failed }
    return bytes
}

// Called only on a Rust blocking worker. All presentation and delegate work stays on the main queue.
@available(macOS 15.0, *)
private final class Ceremony: NSObject, ASAuthorizationControllerDelegate, ASAuthorizationControllerPresentationContextProviding {
    private var controller: ASAuthorizationController?
    private var anchor: NSWindow?
    private var result: ASAuthorizationCredential?
    private let done = DispatchSemaphore(value: 0)

    func run(_ request: @escaping () -> ASAuthorizationRequest) throws -> ASAuthorizationCredential {
        guard !Thread.isMainThread else { throw PasskeyError.failed }
        DispatchQueue.main.async {
            guard let window = NSApp.keyWindow ?? NSApp.mainWindow else { self.done.signal(); return }
            let controller = ASAuthorizationController(authorizationRequests: [request()])
            self.anchor = window
            self.controller = controller
            controller.delegate = self
            controller.presentationContextProvider = self
            controller.performRequests()
        }
        guard done.wait(timeout: .now() + 120) == .success else {
            DispatchQueue.main.async { self.controller?.cancel() }
            throw PasskeyError.failed
        }
        guard let result = result else { throw PasskeyError.failed }
        return result
    }

    func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor { anchor! }
    func authorizationController(controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization) {
        result = authorization.credential
        done.signal()
    }
    func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) { done.signal() }
}

@available(macOS 15.0, *)
private func wrappingKey(_ rp: String, _ id: Data, _ salt: Data) throws -> SymmetricKey {
    let challenge = try randomBytes()
    let credential = try Ceremony().run {
        let request = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: rp).createCredentialAssertionRequest(challenge: challenge)
        request.allowedCredentials = [ASAuthorizationPlatformPublicKeyCredentialDescriptor(credentialID: id)]
        request.userVerificationPreference = .required
        request.prf = .inputValues(.saltInput1(salt))
        return request
    }
    guard let assertion = credential as? ASAuthorizationPlatformPublicKeyCredentialAssertion,
          assertion.credentialID == id, let key = assertion.prf?.first else { throw PasskeyError.failed }
    return key
}

@_cdecl("rv_passkey_available")
public func passkeyAvailable() -> Int32 {
    if #available(macOS 15.0, *) { return relyingParty() == nil ? 0 : 1 }
    return 0
}

@_cdecl("rv_passkey_enroll")
public func passkeyEnroll(_ input: UnsafePointer<UInt8>, _ output: UnsafeMutablePointer<UInt8>, _ capacity: Int) -> Int {
    guard #available(macOS 15.0, *), let rp = relyingParty() else { return 0 }
    do {
        let user = try randomBytes(), challenge = try randomBytes(), salt = try randomBytes()
        let label = "ReleaseVault · " + user.prefix(4).map { String(format: "%02x", $0) }.joined()
        let credential = try Ceremony().run {
            let request = ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: rp)
                .createCredentialRegistrationRequest(challenge: challenge, name: label, userID: user)
            request.userVerificationPreference = .required
            request.prf = .checkForSupport
            return request
        }
        guard let registration = credential as? ASAuthorizationPlatformPublicKeyCredentialRegistration,
              !registration.credentialID.isEmpty, registration.prf?.isSupported == true else { return 0 }
        let id = registration.credentialID
        let key = try wrappingKey(rp, id, salt)
        var raw = Data(bytes: input, count: 32)
        defer { raw.resetBytes(in: 0..<raw.count) }
        guard let sealed = try AES.GCM.seal(raw, using: key).combined else { return 0 }
        let record = try JSONEncoder().encode(PasskeyRecord(rp: rp, id: id, salt: salt, sealed: sealed))
        guard record.count <= capacity else { return 0 }
        record.copyBytes(to: output, count: record.count)
        return record.count
    } catch { return 0 }
}

@_cdecl("rv_passkey_open")
public func passkeyOpen(_ input: UnsafePointer<UInt8>, _ count: Int, _ output: UnsafeMutablePointer<UInt8>) -> Int32 {
    guard #available(macOS 15.0, *), let rp = relyingParty() else { return 0 }
    do {
        let record = try JSONDecoder().decode(PasskeyRecord.self, from: Data(bytes: input, count: count))
        guard record.rp == rp, record.salt.count == 32, !record.id.isEmpty, record.id.count <= 1023 else { return 0 }
        let key = try wrappingKey(rp, record.id, record.salt)
        var raw = try AES.GCM.open(AES.GCM.SealedBox(combined: record.sealed), using: key)
        defer { raw.resetBytes(in: 0..<raw.count) }
        guard raw.count == 32 else { return 0 }
        raw.copyBytes(to: output, count: 32)
        return 1
    } catch { return 0 }
}
