import Foundation

enum OverlayMessage {
    case move(x: Double, y: Double, targetPid: Int?)
    case click(x: Double, y: Double, button: String, count: Int, targetPid: Int?)
    case scroll(x: Double, y: Double, dx: Double, dy: Double, targetPid: Int?)
    case key(text: String?, combo: String?)
    case targetSet(x: Double, y: Double, w: Double, h: Double, targetPid: Int?)
    case targetClear
    case error(x: Double?, y: Double?, code: String, message: String)
    case thinking(Bool)
    case setVisible(Bool)
    case setColor(r: Double, g: Double, b: Double)
    case bye
}

enum ProtocolDecodeError: Error {
    case missingKind
    case unknownKind(String)
    case missingField(String)
    case invalidField(String)
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
        case "scroll":
            return .scroll(
                x: try numericField(dict, "x"),
                y: try numericField(dict, "y"),
                dx: try numericField(dict, "dx"),
                dy: try numericField(dict, "dy"),
                targetPid: optionalIntField(dict, "target_pid")
            )
        case "key":
            let text = try optionalStringField(dict, "text")
            let combo = try optionalStringField(dict, "combo")
            switch (text, combo) {
            case (.some(_), nil), (nil, .some(_)):
                return .key(text: text, combo: combo)
            default:
                throw ProtocolDecodeError.invalidField("text/combo")
            }
        case "target":
            if (dict["clear"] as? Bool) == true {
                return .targetClear
            }
            return .targetSet(
                x: try numericField(dict, "x"),
                y: try numericField(dict, "y"),
                w: try numericField(dict, "w"),
                h: try numericField(dict, "h"),
                targetPid: optionalIntField(dict, "target_pid")
            )
        case "error":
            guard let code = dict["code"] as? String else {
                throw ProtocolDecodeError.missingField("code")
            }
            guard let message = dict["message"] as? String else {
                throw ProtocolDecodeError.missingField("message")
            }
            return .error(
                x: try optionalNumericField(dict, "x"),
                y: try optionalNumericField(dict, "y"),
                code: code,
                message: message
            )
        case "thinking":
            guard let v = dict["thinking"] as? Bool else {
                throw ProtocolDecodeError.missingField("thinking")
            }
            return .thinking(v)
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

    private static func optionalNumericField(_ dict: [String: Any], _ key: String) throws -> Double? {
        guard dict.keys.contains(key) else { return nil }
        return try numericField(dict, key)
    }

    private static func optionalStringField(_ dict: [String: Any], _ key: String) throws -> String? {
        guard let v = dict[key] else { return nil }
        guard let s = v as? String else {
            throw ProtocolDecodeError.invalidField(key)
        }
        return s
    }

    private static func optionalIntField(_ dict: [String: Any], _ key: String) -> Int? {
        guard let v = dict[key] else { return nil }
        if let i = v as? Int { return i }
        if let n = v as? NSNumber { return n.intValue }
        return nil
    }
}
