//! Reads the computer's DNS servers, and (on request, with the OS admin prompt) switches them to
//! the family servers. Undo is in docs/family-dns.md, on purpose not a button.

use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

/// The servers the system uses by default (empty if unknown).
pub fn servers() -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        // `scutil --dns`: the first resolver block is the default one.
        let text = run("scutil", &["--dns"]);
        let first = text.split("resolver #2").next().unwrap_or_default();
        first.lines().filter_map(|l| l.trim().strip_prefix("nameserver[")?.split_once(':').map(|(_, ip)| ip.trim().to_owned())).collect()
    }
    #[cfg(target_os = "linux")]
    {
        let conf = std::fs::read_to_string("/etc/resolv.conf").unwrap_or_default();
        let mut list: Vec<String> = conf.lines().filter_map(|l| l.strip_prefix("nameserver")).map(|s| s.trim().to_owned()).collect();
        if list.iter().any(|s| s.starts_with("127.0.0.53")) {
            // systemd-resolved stub: ask it for the real upstream servers.
            list = run("resolvectl", &["dns"]).lines().filter_map(|l| l.split_once(':')).flat_map(|(_, s)| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>()).collect();
        }
        list
    }
    #[cfg(windows)]
    {
        run("powershell", &["-NoProfile", "-Command", "(Get-DnsClientServerAddress | Where-Object ServerAddresses).ServerAddresses"])
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    Vec::new()
}

/// Switches every network to the family DNS. The OS asks for the admin password first.
/// Linux: no single tool works everywhere, so the settings link to the guide instead.
pub fn enable() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let shell = "networksetup -listallnetworkservices | tail -n +2 | grep -v '^\\\\*' | while IFS= read -r s; do networksetup -setdnsservers \\\"$s\\\" 1.1.1.3 1.0.0.3; done";
        let script = format!("do shell script \"{shell}\" with administrator privileges");
        let ok = Command::new("osascript").args(["-e", &script]).status().map_err(|e| e.to_string())?.success();
        ok.then_some(()).ok_or_else(|| "cancelled".into())
    }
    #[cfg(windows)]
    {
        let inner = "Get-NetAdapter | Where-Object Status -eq Up | ForEach-Object { Set-DnsClientServerAddress -InterfaceIndex $_.ifIndex -ServerAddresses 1.1.1.3,1.0.0.3 }";
        let outer = format!("Start-Process powershell -Verb RunAs -Wait -ArgumentList '-NoProfile','-Command','{inner}'");
        let ok = Command::new("powershell").args(["-NoProfile", "-Command", &outer]).status().map_err(|e| e.to_string())?.success();
        ok.then_some(()).ok_or_else(|| "cancelled".into())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    Err("not automatic on this system, see the guide".into())
}
