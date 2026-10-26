# Reading time: port report

| Part | Status | What changed |
|---|---|---|
| `content_scripts` (content.js, badge.css) | ✅ runs | Unchanged except the line below |
| `chrome.storage.sync` / `.local` | ✅ runs | Supported as is (sync is stored locally) |
| `chrome.runtime.sendMessage` → background.js counter | ✅ rewritten | The content script now adds 1 to `articles` in storage itself |
| `background.service_worker` | ❌ dropped | Its only job moved into content.js |
| `chrome.action.setBadgeText` | ❌ dropped | No toolbar in canbec. The count is still saved in storage |
| `options_page` (words per minute) | TODO | No options pages yet. Default 230 is used; to change it, add `"wpm": 300` to `extension-storage/reading-time.json` in the config folder (while canbec is closed) |

Install: copy this folder to `<config>/extensions/reading-time/` and restart canbec.
