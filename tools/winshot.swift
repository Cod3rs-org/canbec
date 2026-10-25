// Screenshot a window by its owning process name: swift tools/winshot.swift <process-name> <out.png>
import CoreGraphics
import Foundation
let a = CommandLine.arguments
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
guard let w = list.first(where: { ($0[kCGWindowOwnerName as String] as? String) == a[1] && ($0[kCGWindowLayer as String] as? Int) == 0 }),
      let id = w[kCGWindowNumber as String] as? Int else { print("window not found"); exit(1) }
let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
p.arguments = ["-x", "-o", "-l\(id)", a[2]]; try! p.run(); p.waitUntilExit()
