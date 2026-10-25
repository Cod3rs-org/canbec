// Prints the unix time when a window owned by <owner> (and not by an excluded pid) first appears.
// Build: swiftc -O tools/firstwindow.swift -o target/firstwindow
// Usage: firstwindow <owner-name> <excluded-pids-comma> <timeout-s>
import CoreGraphics
import Foundation
let a = CommandLine.arguments
let excluded = Set(a[2].split(separator: ",").compactMap { Int($0) })
let deadline = Date().addingTimeInterval(Double(a[3]) ?? 30)
while Date() < deadline {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
    if let w = list.first(where: { ($0[kCGWindowOwnerName as String] as? String) == a[1]
        && ($0[kCGWindowLayer as String] as? Int) == 0
        && !excluded.contains($0[kCGWindowOwnerPID as String] as? Int ?? -1) }) {
        print(String(format: "%.3f %d", Date().timeIntervalSince1970, w[kCGWindowOwnerPID as String] as? Int ?? 0))
        exit(0)
    }
    usleep(20_000)
}
print("timeout"); exit(1)
