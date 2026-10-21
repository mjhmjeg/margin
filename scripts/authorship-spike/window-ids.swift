// Lists the on-screen windows of one app (by owner name) as "<CGWindowID>\t<layer>\t<title>",
// so a screenshot can be taken of that window alone (screencapture -l<id>), never of the whole
// desktop. Public CoreGraphics API only. Titles are empty without Screen Recording permission;
// the ids still work.
//
// usage: swift window-ids.swift "Microsoft Word"
import CoreGraphics
import Foundation

let owner = CommandLine.arguments.dropFirst().first ?? "Microsoft Word"
let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
    FileHandle.standardError.write("cannot list windows\n".data(using: .utf8) ?? Data())
    exit(1)
}
for w in windows {
    guard (w[kCGWindowOwnerName as String] as? String) == owner, let id = w[kCGWindowNumber as String] as? Int else { continue }
    let layer = (w[kCGWindowLayer as String] as? Int) ?? 0
    let title = (w[kCGWindowName as String] as? String) ?? ""
    print("\(id)\t\(layer)\t\(title)")
}
