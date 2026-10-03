// A25 — read (and, for OUR notice only, press) macOS Notification Center
// banners through Accessibility, for tools/walk/a25_live_macos.py.
//
//   a25_nc dump              every element of the NotificationCenter process (role, texts, frame, actions)
//   a25_nc find <text>       JSON frames of the banners whose texts contain <text>
//   a25_nc press <text>      AXPress the banner whose texts contain <text> (the notice's default action)
//   a25_nc click <text>      a real pointer click at the centre of that banner (macOS lets an app come
//                            forward only for user input: an AXPress delivers the click but grants no
//                            activation). A hit-test first proves the point is on OUR notice; the
//                            pointer is put back after
//   a25_nc front <bundle-id>  the frontmost app, and that app's own state (active, hidden, activation policy)
//   a25_nc center open|close|state
//                            open / close the Notification Center panel (the menu bar clock), or report it;
//                            macOS presents no banner while the display is shared or mirrored, but the
//                            notice is still listed there, pressable the same way
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

if CommandLine.arguments.count > 2, CommandLine.arguments[1] == "front" {
    let front = NSWorkspace.shared.frontmostApplication?.bundleIdentifier ?? ""
    let app = NSRunningApplication.runningApplications(withBundleIdentifier: CommandLine.arguments[2]).first
    let policy = app.map { $0.activationPolicy == .regular ? "regular" : $0.activationPolicy == .accessory ? "accessory" : "prohibited" } ?? "none"
    print("{\"front\": \"\(front)\", \"active\": \(app?.isActive ?? false), \"hidden\": \(app?.isHidden ?? false), \"policy\": \"\(policy)\"}")
    exit(0)
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
case "find", "press", "click":
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
    } else if cmd == "click", let (e, f, _) = hits.first {
        let point = CGPoint(x: f.midX, y: f.midY)
        var under: AXUIElement?
        AXUIElementCopyElementAtPosition(AXUIElementCreateSystemWide(), Float(point.x), Float(point.y), &under)
        var cursor = under
        var ours = false
        for _ in 0..<16 {
            guard let c = cursor else { break }
            if CFEqual(c, e) { ours = true; break }
            cursor = attr(c, kAXParentAttribute as String).map { $0 as! AXUIElement }
        }
        guard ours else {
            print("{\"clicked\": false, \"error\": \"the point is not on our notice\"}")
            exit(1)
        }
        let back = CGEvent(source: nil)?.location ?? point
        let source = CGEventSource(stateID: .hidSystemState)
        for type in [CGEventType.mouseMoved, .leftMouseDown, .leftMouseUp] {
            CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: point, mouseButton: .left)?
                .post(tap: .cghidEventTap)
            usleep(80_000)
        }
        usleep(300_000)
        CGEvent(mouseEventSource: source, mouseType: .mouseMoved, mouseCursorPosition: back, mouseButton: .left)?
            .post(tap: .cghidEventTap)
        print("{\"clicked\": true, \"x\": \(f.minX), \"y\": \(f.minY), \"w\": \(f.width), \"h\": \(f.height)}")
    } else if cmd == "press", let (e, f, _) = hits.first {
        let r = AXUIElementPerformAction(e, kAXPressAction as CFString)
        print("{\"pressed\": \(r == .success), \"x\": \(f.minX), \"y\": \(f.minY), \"w\": \(f.width), \"h\": \(f.height)}")
    } else {
        print("{\"pressed\": false, \"error\": \"no banner carries the text\"}")
        exit(1)
    }
case "center":
    // The panel is the NotificationCenter process's system-dialog window; the clock toggles it.
    func panelOpen() -> Bool {
        ((attr(root, kAXWindowsAttribute as String) as? [AXUIElement]) ?? [])
            .contains { text($0, kAXSubroleAttribute as String) == "AXSystemDialog" }
    }
    let open = panelOpen()
    if needle == "state" || (needle == "open") == open {
        print("{\"open\": \(open)}")
        break
    }
    guard let cc = NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.controlcenter").first else {
        print("{\"error\": \"no Control Center process\"}")
        exit(2)
    }
    var clock: AXUIElement?
    walk(AXUIElementCreateApplication(cc.processIdentifier), 0) { e, _ in
        if clock == nil, text(e, kAXIdentifierAttribute as String) == "com.apple.menuextra.clock" { clock = e }
    }
    guard let clock else {
        print("{\"error\": \"no menu bar clock\"}")
        exit(1)
    }
    _ = AXUIElementPerformAction(clock, kAXPressAction as CFString)
    var now = open
    for _ in 0..<20 where now == open {
        usleep(100_000)
        now = panelOpen()
    }
    print("{\"open\": \(now)}")
default:
    print("usage: a25_nc dump | find <text> | press <text> | click <text> | center open|close|state")
    exit(64)
}
