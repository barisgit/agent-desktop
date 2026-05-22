import Foundation
import Darwin

final class SocketServer {
    private let path: String
    private let onMessage: (OverlayMessage) -> Void
    private var listenFd: Int32 = -1
    private var listenSource: DispatchSourceRead?
    private var clientFd: Int32 = -1
    private var clientSource: DispatchSourceRead?
    private var buffer = Data()
    private let queue = DispatchQueue(label: "cursor-overlay.socket")

    init(path: String, onMessage: @escaping (OverlayMessage) -> Void) {
        self.path = path
        self.onMessage = onMessage
    }

    func start() throws {
        unlink(path)

        listenFd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard listenFd >= 0 else { throw NSError(domain: "co.socket", code: 1, userInfo: [NSLocalizedDescriptionKey: "socket() failed: errno=\(errno)"]) }

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = Array(path.utf8)
        guard pathBytes.count < MemoryLayout.size(ofValue: addr.sun_path) else {
            close(listenFd); listenFd = -1
            throw NSError(domain: "co.socket", code: 2, userInfo: [NSLocalizedDescriptionKey: "socket path too long"])
        }
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            ptr.withMemoryRebound(to: Int8.self, capacity: pathBytes.count) { dest in
                for (i, b) in pathBytes.enumerated() {
                    dest[i] = Int8(bitPattern: b)
                }
                dest[pathBytes.count] = 0
            }
        }
        let len = socklen_t(MemoryLayout<sockaddr_un>.size)
        let bindResult = withUnsafePointer(to: &addr) { ptr -> Int32 in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { saPtr in
                bind(listenFd, saPtr, len)
            }
        }
        guard bindResult == 0 else {
            let e = errno
            close(listenFd); listenFd = -1
            throw NSError(domain: "co.socket", code: 3, userInfo: [NSLocalizedDescriptionKey: "bind() failed: errno=\(e)"])
        }
        guard listen(listenFd, 4) == 0 else {
            let e = errno
            close(listenFd); listenFd = -1
            throw NSError(domain: "co.socket", code: 4, userInfo: [NSLocalizedDescriptionKey: "listen() failed: errno=\(e)"])
        }

        let src = DispatchSource.makeReadSource(fileDescriptor: listenFd, queue: queue)
        src.setEventHandler { [weak self] in self?.acceptClient() }
        src.resume()
        listenSource = src

        fputs("cursor-overlay: socket listening at \(path)\n", stderr)
    }

    private func acceptClient() {
        var clientAddr = sockaddr()
        var len: socklen_t = socklen_t(MemoryLayout<sockaddr>.size)
        let fd = accept(listenFd, &clientAddr, &len)
        if fd < 0 { return }
        if clientFd >= 0 {
            close(clientFd)
            clientSource?.cancel()
            clientSource = nil
            buffer.removeAll()
        }
        clientFd = fd
        let src = DispatchSource.makeReadSource(fileDescriptor: fd, queue: queue)
        src.setEventHandler { [weak self] in self?.readFromClient() }
        src.setCancelHandler { [weak self] in
            if let self = self, self.clientFd == fd {
                close(self.clientFd)
                self.clientFd = -1
            }
        }
        src.resume()
        clientSource = src
        fputs("cursor-overlay: client connected (fd=\(fd))\n", stderr)
    }

    private func readFromClient() {
        guard clientFd >= 0 else { return }
        var chunk = [UInt8](repeating: 0, count: 4096)
        let n = read(clientFd, &chunk, chunk.count)
        if n <= 0 {
            fputs("cursor-overlay: client disconnected\n", stderr)
            clientSource?.cancel()
            clientSource = nil
            buffer.removeAll()
            return
        }
        buffer.append(contentsOf: chunk.prefix(n))
        drainBuffer()
    }

    private func drainBuffer() {
        while let nl = buffer.firstIndex(of: 0x0a) {
            let line = buffer.subdata(in: 0..<nl)
            buffer.removeSubrange(0...nl)
            guard !line.isEmpty else { continue }
            do {
                let msg = try ProtocolDecoder.decode(line: line)
                onMessage(msg)
            } catch ProtocolDecodeError.unknownKind(let kind) {
                writeStderr("cursor-overlay: unknownKind: \(kind)\n")
            } catch {
                fputs("cursor-overlay: decode error: \(error)\n", stderr)
            }
        }
    }

    private func writeStderr(_ line: String) {
        guard let data = line.data(using: .utf8) else { return }
        FileHandle.standardError.write(data)
    }

    func stop() {
        listenSource?.cancel()
        clientSource?.cancel()
        if clientFd >= 0 { close(clientFd); clientFd = -1 }
        if listenFd >= 0 { close(listenFd); listenFd = -1 }
        unlink(path)
    }
}
