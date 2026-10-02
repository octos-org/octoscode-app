// A25 — read (and, for OUR notice only, press) macOS Notification Center
// banners through Accessibility, for tools/walk/a25_live_macos.py.
//
//   a25_nc dump              every element of the NotificationCenter process (role, texts, frame, actions)
//   a25_nc find <text>       JSON frames of the banners whose texts contain <text>
//   a25_nc press <text>      AXPress the banner whose texts contain <text> (the notice's default action)
//
// It never presses anything whose texts do not contain <text>; the live script
// passes the app's own notice text, so a system permission alert (which carries
// none of it) can be read and captured but never answered.
import ApplicationServices
import Cocoa

func attr(_ e: AXUIElement, _ name: String) -> AnyObject? {
    var v: AnyObject?
    return AXUIElementCopyAttributeValue(e, name as CFString, &v) == .success ? v : nil
}

func text(_ e: AXUIElement, _ name: String) -> String {
    if let s = attr(e, name) as? String { return s }
    return ""
}

func children(_ e: AXUIElement) -> [AXUIElement] {
    (attr(e, kAXChildrenAttribute as String) as? [AXUIElement]) ?? []
}

func frame(_ e: AXUIElement) -> CGRect? {
    guard let p = attr(e, kAXPositionAttribute as String), let s = attr(e, kAXSizeAttribute as String) else { return nil }
    var point = CGPoint.zero
    var size = CGSize.zero
    AXValueGetValue(p as! AXValue, .cgPoint, &point)
    AXValueGetValue(s as! AXValue, .cgSize, &size)
    return CGRect(origin: point, size: size)
}

func actions(_ e: AXUIElement) -> [String] {
    var names: CFArray?
    return AXUIElementCopyActionNames(e, &names) == .success ? ((names as? [String]) ?? []) : []
}

/// Every text an element and its subtree carry (a banner's title and body
/// are usually static-text children of the pressable group).
func texts(_ e: AXUIElement, depth: Int = 0) -> [String] {
    var out = [kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute, kAXIdentifierAttribute]
        .map { text(e, $0 as String) }.filter { !$0.isEmpty }
    if depth < 8 { for c in children(e) { out += texts(c, depth: depth + 1) } }
    return out
}

func walk(_ e: AXUIElement, _ depth: Int, _ visit: (AXUIElement, Int) -> Void) {
    visit(e, depth)
    if depth < 14 { for c in children(e) { walk(c, depth + 1, visit) } }
}

guard let nc = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.notificationcenterui").first else {
    print("{\"error\": \"no NotificationCenter process\"}")
    exit(2)
}
let root = AXUIElementCreateApplication(nc.processIdentifier)
let args = CommandLine.arguments
let cmd = args.count > 1 ? args[1] : "dump"
let needle = args.count > 2 ? args[2] : ""

switch cmd {
case "dump":
    walk(root, 0) { e, d in
        let f = frame(e).map { "\(Int($0.minX)),\(Int($0.minY)),\(Int($0.width)),\(Int($0.height))" } ?? "-"
        let own = [kAXTitleAttribute, kAXDescriptionAttribute, kAXValueAttribute]
            .map { text(e, $0 as String) }.filter { !$0.isEmpty }.joined(separator: " | ")
        print(String(repeating: "  ", count: d) + "\(text(e, kAXRoleAttribute as String)) [\(text(e, kAXSubroleAttribute as String))] {\(f)} \(own.prefix(140)) \(actions(e))")
    }
case "find", "press":
    // The smallest pressable element whose subtree carries the needle.
    var hits: [(AXUIElement, CGRect, Int)] = []
    walk(root, 0) { e, d in
        guard !needle.isEmpty, actions(e).contains(kAXPressAction as String), let f = frame(e) else { return }
        if texts(e).contains(where: { $0.contains(needle) }) { hits.append((e, f, d)) }
    }
    hits.sort { $0.2 > $1.2 }
    if cmd == "find" {
        let rows = hits.map { "{\"x\": \($0.1.minX), \"y\": \($0.1.minY), \"w\": \($0.1.width), \"h\": \($0.1.height)}" }
        print("[" + rows.joined(separator: ", ") + "]")
    } else if let (e, f, _) = hits.first {
        let r = AXUIElementPerformAction(e, kAXPressAction as CFString)
        print("{\"pressed\": \(r == .success), \"x\": \(f.minX), \"y\": \(f.minY), \"w\": \(f.width), \"h\": \(f.height)}")
    } else {
        print("{\"pressed\": false, \"error\": \"no banner carries the text\"}")
        exit(1)
    }
default:
    print("usage: a25_nc dump | find <text> | press <text>")
    exit(64)
}
