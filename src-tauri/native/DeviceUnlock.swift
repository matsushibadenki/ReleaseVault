// The enclave key can be used only after the OS satisfies its user-presence policy.
import CryptoKit
import Foundation
import LocalAuthentication
import Security

private struct DeviceEnvelope: Codable {
    let enclave: Data
    let peer: Data
    let sealed: Data
}

private func wrappingKey(_ agreement: SharedSecret) -> SymmetricKey {
    agreement.hkdfDerivedSymmetricKey(using: SHA256.self,
        salt: Data("ReleaseVault.device-unlock.v1".utf8),
        sharedInfo: Data("vault-key".utf8), outputByteCount: 32)
}

@_cdecl("rv_device_available")
public func available() -> Int32 {
    return SecureEnclave.isAvailable && LAContext().canEvaluatePolicy(.deviceOwnerAuthentication, error: nil) ? 1 : 0
}

// Returns the number of output bytes, or zero on failure. The output buffer belongs to Rust.
@_cdecl("rv_device_enroll")
public func enroll(_ input: UnsafePointer<UInt8>, _ output: UnsafeMutablePointer<UInt8>, _ capacity: Int) -> Int {
    do {
        guard let access = SecAccessControlCreateWithFlags(nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
                [.userPresence, .privateKeyUsage], nil) else { return 0 }
        let enclave = try SecureEnclave.P256.KeyAgreement.PrivateKey(accessControl: access)
        let ephemeral = P256.KeyAgreement.PrivateKey()
        let agreement = try ephemeral.sharedSecretFromKeyAgreement(with: enclave.publicKey)
        let box = try AES.GCM.seal(Data(bytes: input, count: 32), using: wrappingKey(agreement))
        guard let sealed = box.combined else { return 0 }
        let encoded = try JSONEncoder().encode(DeviceEnvelope(enclave: enclave.dataRepresentation,
            peer: ephemeral.publicKey.x963Representation, sealed: sealed))
        guard encoded.count <= capacity else { return 0 }
        encoded.copyBytes(to: output, count: encoded.count)
        return encoded.count
    } catch { return 0 }
}

@_cdecl("rv_device_open")
public func unwrap(_ input: UnsafePointer<UInt8>, _ count: Int, _ output: UnsafeMutablePointer<UInt8>) -> Int32 {
    do {
        let record = try JSONDecoder().decode(DeviceEnvelope.self, from: Data(bytes: input, count: count))
        let context = LAContext()
        defer { context.invalidate() }
        context.localizedReason = NSLocalizedString("ReleaseVault", comment: "Vault unlock")
        let enclave = try SecureEnclave.P256.KeyAgreement.PrivateKey(dataRepresentation: record.enclave, authenticationContext: context)
        let peer = try P256.KeyAgreement.PublicKey(x963Representation: record.peer)
        let agreement = try enclave.sharedSecretFromKeyAgreement(with: peer)
        let box = try AES.GCM.SealedBox(combined: record.sealed)
        var bytes = try AES.GCM.open(box, using: wrappingKey(agreement))
        defer { bytes.resetBytes(in: 0..<bytes.count) }
        guard bytes.count == 32 else { return 0 }
        bytes.copyBytes(to: output, count: 32)
        return 1
    } catch { return 0 }
}
