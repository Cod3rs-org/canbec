# Family DNS: block adult sites in every app

canbec blocks adult sites by itself, and you can't switch that off. Other browsers and apps on
the same computer can still open them. **Cloudflare's "1.1.1.1 for Families"** DNS fixes that for
the whole computer: it refuses to resolve adult (and malware) domains for every app.

- Servers: `1.1.1.3` and `1.0.0.3` (IPv6: `2606:4700:4700::1113`, `2606:4700:4700::1003`)
- Free, no account. Cloudflare's privacy policy: no selling of data, logs wiped within 25 hours.
- canbec shows **System-wide: on** in Settings → Adult content when every DNS server is a family one.

## Turn it on

**macOS / Windows:** Settings → Adult content → **Turn on for this computer…** The system asks
for your admin password, then every network uses the family servers.

**By hand:**

| OS | Steps |
|---|---|
| macOS | System Settings → Network → (your network) → Details → DNS → replace the servers with `1.1.1.3` and `1.0.0.3` |
| Windows | Settings → Network & internet → (adapter) → DNS server assignment → Edit → Manual → IPv4: `1.1.1.3`, `1.0.0.3` |
| Linux (NetworkManager) | `nmcli con mod "<connection>" ipv4.dns "1.1.1.3 1.0.0.3" ipv4.ignore-auto-dns yes && nmcli con up "<connection>"` |
| Router (all devices at home) | Set the router's DNS to `1.1.1.3` / `1.0.0.3` |

## Undo (on purpose not a button)

A one-click undo would make it too easy in a weak moment. To go back, do it by hand:

| OS | Steps |
|---|---|
| macOS | `sudo networksetup -setdnsservers Wi-Fi empty` (and the same for other network names from `networksetup -listallnetworkservices`) |
| Windows (admin PowerShell) | `Get-NetAdapter \| ForEach-Object { Set-DnsClientServerAddress -InterfaceIndex $_.ifIndex -ResetServerAddresses }` |
| Linux (NetworkManager) | `nmcli con mod "<connection>" ipv4.dns "" ipv4.ignore-auto-dns no && nmcli con up "<connection>"` |

## Limits

- A VPN or an app with its own DNS-over-HTTPS (some browsers do this) goes around system DNS.
- Anyone with the admin password can change it back. It's a speed bump, not a lock.
