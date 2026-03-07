import CoreGraphics
import Foundation

struct WindowSummary: Codable {
  let id: Int
  let ownerName: String
  let windowName: String
  let x: Int
  let y: Int
  let width: Int
  let height: Int
  let layer: Int
  let onScreen: Bool
}

/**
 Summary
 Convert an arbitrary Core Foundation field into a printable string.

 Inputs
 `value` as any optional value from the Quartz window dictionary.

 Outputs
 A normalized string representation.

 Side effects
 None.

 Error handling
 Returns an empty string when the field is absent or cannot be represented.

 Ties to other methods
 Used by `listWindows` when decoding Quartz window metadata.

 Why this exists
 Quartz window dictionaries mix Core Foundation and Swift values, so string normalization must be centralized.
 */
func stringValue(_ value: Any?) -> String {
  if let text = value as? String {
    return text
  }
  if let number = value as? NSNumber {
    return number.stringValue
  }
  return ""
}

/**
 Summary
 Read the current visible window list from Quartz and map it into serializable summaries.

 Inputs
 None.

 Outputs
 An array of `WindowSummary` values.

 Side effects
 Reads global window metadata from the current macOS session.

 Error handling
 Returns an empty array when Quartz does not provide window data.

 Ties to other methods
 Used by `main` to provide the `list` command output.

 Why this exists
 The desktop bridge needs stable machine-readable window metadata for screenshots and targeting.
 */
func listWindows() -> [WindowSummary] {
  let options = CGWindowListOption(arrayLiteral: .optionOnScreenOnly, .excludeDesktopElements)
  guard let rawList = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
    return []
  }

  return rawList.compactMap { entry in
    guard
      let windowId = entry[kCGWindowNumber as String] as? NSNumber,
      let bounds = entry[kCGWindowBounds as String] as? [String: Any],
      let x = bounds["X"] as? NSNumber,
      let y = bounds["Y"] as? NSNumber,
      let width = bounds["Width"] as? NSNumber,
      let height = bounds["Height"] as? NSNumber
    else {
      return nil
    }

    return WindowSummary(
      id: windowId.intValue,
      ownerName: stringValue(entry[kCGWindowOwnerName as String]),
      windowName: stringValue(entry[kCGWindowName as String]),
      x: x.intValue,
      y: y.intValue,
      width: width.intValue,
      height: height.intValue,
      layer: (entry[kCGWindowLayer as String] as? NSNumber)?.intValue ?? 0,
      onScreen: (entry[kCGWindowIsOnscreen as String] as? Bool) ?? true
    )
  }
}

/**
 Summary
 Encode the current window list as compact JSON and write it to stdout.

 Inputs
 Command-line arguments, which are currently ignored.

 Outputs
 Exit status zero on success and a JSON payload on stdout.

 Side effects
 Writes to stdout and stderr.

 Error handling
 Prints a contextual error to stderr and exits non-zero when JSON encoding fails.

 Ties to other methods
 Calls `listWindows`.

 Why this exists
 The Python desktop bridge needs a fast native helper that exposes Quartz window metadata without extra Python packages.
 */
func main() {
  let encoder = JSONEncoder()
  encoder.outputFormatting = [.withoutEscapingSlashes]

  do {
    let data = try encoder.encode(listWindows())
    FileHandle.standardOutput.write(data)
  } catch {
    let message = "[window_info.swift::main] Failed to encode window list: \(error)\n"
    if let data = message.data(using: .utf8) {
      FileHandle.standardError.write(data)
    }
    exit(1)
  }
}

main()
