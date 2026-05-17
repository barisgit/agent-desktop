import Foundation

enum OverlayMessage {
    case move(x: Double, y: Double, targetPid: Int?)
    case click(x: Double, y: Double, button: String, count: Int, targetPid: Int?)
    case setVisible(Bool)
    case setColor(r: Double, g: Double, b: Double)
    case setThinking(Bool)
    case bye
}

enum ProtocolDecodeError: Error {
    case missingKind
    case unknownKind(String)
    case missingField(String)
    case invalidJson
}

struct ProtocolDecoder {
    static func decode(line: Data) throws -> OverlayMessage {
        guard let obj = try? JSONSerialization.jsonObject(with: line, options: []),
              let dict = obj as? [String: Any]
        else {
            throw ProtocolDecodeError.invalidJson
        }
        guard let kind = dict["kind"] as? String else {
            throw ProtocolDecodeError.missingKind
        }
        switch kind {
        case "move":
            return .move(
                x: try numericField(dict, "x"),
                y: try numericField(dict, "y"),
                targetPid: optionalIntField(dict, "target_pid")
            )
        case "click":
            let button = (dict["button"] as? String) ?? "left"
            let count = (dict["count"] as? Int) ?? 1
            return .click(
                x: try numericField(dict, "x"),
                y: try numericField(dict, "y"),
                button: button,
                count: count,
                targetPid: optionalIntField(dict, "target_pid")
            )
        case "set_visible":
            guard let v = dict["visible"] as? Bool else {
                throw ProtocolDecodeError.missingField("visible")
            }
            return .setVisible(v)
        case "set_color":
            return .setColor(
                r: try numericField(dict, "r"),
                g: try numericField(dict, "g"),
                b: try numericField(dict, "b")
            )
        case "set_thinking":
            guard let v = dict["thinking"] as? Bool else {
                throw ProtocolDecodeError.missingField("thinking")
            }
            return .setThinking(v)
        case "bye":
            return .bye
        default:
            throw ProtocolDecodeError.unknownKind(kind)
        }
    }

    private static func numericField(_ dict: [String: Any], _ key: String) throws -> Double {
        guard let v = dict[key] else {
            throw ProtocolDecodeError.missingField(key)
        }
        if let n = v as? NSNumber {
            return n.doubleValue
        }
        if let d = v as? Double { return d }
        if let i = v as? Int { return Double(i) }
        throw ProtocolDecodeError.missingField(key)
    }

    private static func optionalIntField(_ dict: [String: Any], _ key: String) -> Int? {
        guard let v = dict[key] else { return nil }
        if let i = v as? Int { return i }
        if let n = v as? NSNumber { return n.intValue }
        return nil
    }
}
