// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::process::Command;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Deserialize)]
struct Printer {
    Name: String,
}

#[derive(Deserialize)]
struct Device {
    Name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkPrinter {
    pub name: String,
    pub ip_address: String,
    pub port: u16,
    pub model: Option<String>,
    pub status: String,
}

#[tauri::command]
fn process_text(text: String) -> String {
    if text.is_empty() {
        String::new()
    } else {
        format!("Hello, {}!", text)
    }
}

#[tauri::command]
fn list_printers() -> Result<Vec<String>, String> {
    // Try PowerShell first
    eprintln!("Attempting PowerShell Get-Printer command");
    let output = std::process::Command::new("powershell")
        .arg("-Command")
        .arg("Get-Printer | Select-Object -Property Name | ConvertTo-Json -Compress")
        .output();

    let output = match output {
        Ok(output) => output,
        Err(e) => {
            eprintln!("PowerShell Get-Printer failed: {}", e);
            // Fallback to wmic
            eprintln!("Attempting wmic printer command");
            let wmic_output = std::process::Command::new("wmic")
                .arg("printer")
                .arg("get")
                .arg("name")
                .output()
                .map_err(|e| format!("Failed to execute wmic command: {}", e))?;

            if !wmic_output.status.success() {
                return Err(format!("wmic command failed: {}", String::from_utf8_lossy(&wmic_output.stderr)));
            }

            let raw_output = String::from_utf8_lossy(&wmic_output.stdout);
            eprintln!("Raw wmic output: {}", raw_output);

            let printer_names: Vec<String> = raw_output
                .lines()
                .skip(1) // Skip header
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .collect();

            eprintln!("Parsed printer names (wmic): {:?}", printer_names);
            return Ok(printer_names);
        }
    };

    if !output.status.success() {
        return Err(format!("PowerShell command failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let raw_output = String::from_utf8_lossy(&output.stdout);
    eprintln!("Raw PowerShell output: {}", raw_output);

    if raw_output.trim().is_empty() {
        eprintln!("No printers detected by PowerShell");
        return Ok(vec![]);
    }

    let printer_names: Vec<String> = match serde_json::from_slice::<Vec<Printer>>(&output.stdout) {
        Ok(printers) => printers.into_iter().map(|p| p.Name).collect(),
        Err(_) => match serde_json::from_slice::<Printer>(&output.stdout) {
            Ok(printer) => vec![printer.Name],
            Err(e) => return Err(format!("Failed to parse JSON: {}. Raw output: {}", e, raw_output)),
        },
    };

    eprintln!("Parsed printer names: {:?}", printer_names);
    Ok(printer_names)
}

#[tauri::command]
fn list_all_printers() -> Result<Vec<NetworkPrinter>, String> {
    let mut all_printers = Vec::new();
    
    // Get locally installed printers first
    match list_local_printers() {
        Ok(local_printers) => {
            for printer_name in local_printers {
                all_printers.push(NetworkPrinter {
                    name: printer_name,
                    ip_address: "local".to_string(),
                    port: 0,
                    model: None,
                    status: "Local".to_string(),
                });
            }
        }
        Err(e) => eprintln!("Failed to get local printers: {}", e),
    }
    
    // Discover network printers
    match discover_network_printers() {
        Ok(network_printers) => {
            all_printers.extend(network_printers);
        }
        Err(e) => eprintln!("Failed to discover network printers: {}", e),
    }
    
    Ok(all_printers)
}

fn list_local_printers() -> Result<Vec<String>, String> {
    // Try PowerShell first
    eprintln!("Attempting PowerShell Get-Printer command");
    let output = Command::new("powershell")
        .arg("-Command")
        .arg("Get-Printer | Select-Object -Property Name | ConvertTo-Json -Compress")
        .output();

    let output = match output {
        Ok(output) => output,
        Err(e) => {
            eprintln!("PowerShell Get-Printer failed: {}", e);
            // Fallback to wmic
            eprintln!("Attempting wmic printer command");
            let wmic_output = Command::new("wmic")
                .arg("printer")
                .arg("get")
                .arg("name")
                .output()
                .map_err(|e| format!("Failed to execute wmic command: {}", e))?;

            if !wmic_output.status.success() {
                return Err(format!("wmic command failed: {}", String::from_utf8_lossy(&wmic_output.stderr)));
            }

            let raw_output = String::from_utf8_lossy(&wmic_output.stdout);
            eprintln!("Raw wmic output: {}", raw_output);

            let printer_names: Vec<String> = raw_output
                .lines()
                .skip(1) // Skip header
                .filter(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_string())
                .collect();

            eprintln!("Parsed printer names (wmic): {:?}", printer_names);
            return Ok(printer_names);
        }
    };

    if !output.status.success() {
        return Err(format!("PowerShell command failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let raw_output = String::from_utf8_lossy(&output.stdout);
    eprintln!("Raw PowerShell output: {}", raw_output);

    if raw_output.trim().is_empty() {
        eprintln!("No printers detected by PowerShell");
        return Ok(vec![]);
    }

    let printer_names: Vec<String> = match serde_json::from_slice::<Vec<Printer>>(&output.stdout) {
        Ok(printers) => printers.into_iter().map(|p| p.Name).collect(),
        Err(_) => match serde_json::from_slice::<Printer>(&output.stdout) {
            Ok(printer) => vec![printer.Name],
            Err(e) => return Err(format!("Failed to parse JSON: {}. Raw output: {}", e, raw_output)),
        },
    };

    eprintln!("Parsed printer names: {:?}", printer_names);
    Ok(printer_names)
}

fn discover_network_printers() -> Result<Vec<NetworkPrinter>, String> {
    let mut network_printers = Vec::new();
    
    eprintln!("Starting network printer discovery...");
    
    // Method 1: Use Windows NET VIEW command
    eprintln!("Trying NET VIEW discovery...");
    match discover_printers_net_view() {
        Ok(net_view_printers) => {
            eprintln!("NET VIEW found {} printers", net_view_printers.len());
            network_printers.extend(net_view_printers);
        }
        Err(e) => eprintln!("NET VIEW failed: {}", e),
    }
    
    // Method 2: Use PowerShell WMI to find network printers
    eprintln!("Trying WMI discovery...");
    match discover_printers_wmi() {
        Ok(wmi_printers) => {
            eprintln!("WMI found {} printers", wmi_printers.len());
            network_printers.extend(wmi_printers);
        }
        Err(e) => eprintln!("WMI failed: {}", e),
    }
    
    // Method 3: Use PowerShell to find shared printers on network
    eprintln!("Trying PowerShell network discovery...");
    match discover_printers_powershell_network() {
        Ok(ps_printers) => {
            eprintln!("PowerShell network found {} printers", ps_printers.len());
            network_printers.extend(ps_printers);
        }
        Err(e) => eprintln!("PowerShell network failed: {}", e),
    }
    
    // Method 4: Port scan common printer ports on local network (limited range)
    eprintln!("Trying port scan discovery...");
    match discover_printers_port_scan() {
        Ok(scanned_printers) => {
            eprintln!("Port scan found {} printers", scanned_printers.len());
            network_printers.extend(scanned_printers);
        }
        Err(e) => eprintln!("Port scan failed: {}", e),
    }
    
    // Remove duplicates based on IP address
    let mut unique_printers = Vec::new();
    let mut seen_ips = HashSet::new();
    
    for printer in network_printers {
        if seen_ips.insert(printer.ip_address.clone()) {
            unique_printers.push(printer);
        }
    }
    
    eprintln!("Total unique network printers found: {}", unique_printers.len());
    Ok(unique_printers)
}

fn discover_printers_net_view() -> Result<Vec<NetworkPrinter>, String> {
    let output = Command::new("net")
        .arg("view")
        .output()
        .map_err(|e| format!("Failed to execute net view: {}", e))?;
    
    if !output.status.success() {
        return Ok(Vec::new());
    }
    
    let raw_output = String::from_utf8_lossy(&output.stdout);
    let mut printers = Vec::new();
    
    for line in raw_output.lines() {
        if line.contains("Print") || line.contains("Printer") {
            if let Some(name) = extract_computer_name(line) {
                if let Ok(ip) = resolve_hostname(&name) {
                    printers.push(NetworkPrinter {
                        name: name.clone(),
                        ip_address: ip,
                        port: 515, // Default LPR port
                        model: None,
                        status: "Network".to_string(),
                    });
                }
            }
        }
    }
    
    Ok(printers)
}

fn discover_printers_wmi() -> Result<Vec<NetworkPrinter>, String> {
    let output = Command::new("powershell")
        .arg("-Command")
        .arg("Get-WmiObject -Class Win32_Printer | Where-Object {$_.Network -eq $true} | Select-Object Name, PortName, DriverName | ConvertTo-Json -Compress")
        .output()
        .map_err(|e| format!("Failed to execute PowerShell WMI: {}", e))?;
    
    if !output.status.success() {
        return Ok(Vec::new());
    }
    
    let raw_output = String::from_utf8_lossy(&output.stdout);
    if raw_output.trim().is_empty() {
        return Ok(Vec::new());
    }
    
    let mut printers = Vec::new();
    
    // Try to parse as array first, then as single object
    if let Ok(wmi_printers) = serde_json::from_str::<Vec<serde_json::Value>>(&raw_output) {
        for printer in wmi_printers {
            if let (Some(name), Some(port_name)) = (
                printer["Name"].as_str(),
                printer["PortName"].as_str(),
            ) {
                printers.push(NetworkPrinter {
                    name: name.to_string(),
                    ip_address: extract_ip_from_port(port_name).unwrap_or_else(|| port_name.to_string()),
                    port: 9100, // Default IPP port
                    model: printer["DriverName"].as_str().map(|s| s.to_string()),
                    status: "Network (WMI)".to_string(),
                });
            }
        }
    } else if let Ok(printer) = serde_json::from_str::<serde_json::Value>(&raw_output) {
        if let (Some(name), Some(port_name)) = (
            printer["Name"].as_str(),
            printer["PortName"].as_str(),
        ) {
            printers.push(NetworkPrinter {
                name: name.to_string(),
                ip_address: extract_ip_from_port(port_name).unwrap_or_else(|| port_name.to_string()),
                port: 9100,
                model: printer["DriverName"].as_str().map(|s| s.to_string()),
                status: "Network (WMI)".to_string(),
            });
        }
    }
    
    Ok(printers)
}

fn discover_printers_powershell_network() -> Result<Vec<NetworkPrinter>, String> {
    // Use PowerShell to find shared printers on the network
    let output = Command::new("powershell")
        .arg("-Command")
        .arg("Get-WmiObject -Class Win32_Printer | Where-Object {$_.Shared -eq $true -and $_.Network -eq $true} | Select-Object Name, ShareName, PortName, DriverName | ConvertTo-Json -Compress")
        .output()
        .map_err(|e| format!("Failed to execute PowerShell network discovery: {}", e))?;
    
    if !output.status.success() {
        // Try alternative approach with Get-Printer
        let alt_output = Command::new("powershell")
            .arg("-Command")
            .arg("Get-Printer | Where-Object {$_.Shared -eq $true -or $_.Type -eq 'Connection'} | Select-Object Name, PortName, DriverName | ConvertTo-Json -Compress")
            .output()
            .map_err(|e| format!("Failed to execute alternative PowerShell command: {}", e))?;
        
        if !alt_output.status.success() {
            return Ok(Vec::new());
        }
        
        return parse_powershell_printer_output(&String::from_utf8_lossy(&alt_output.stdout));
    }
    
    let raw_output = String::from_utf8_lossy(&output.stdout);
    parse_powershell_printer_output(&raw_output)
}

fn parse_powershell_printer_output(raw_output: &str) -> Result<Vec<NetworkPrinter>, String> {
    if raw_output.trim().is_empty() {
        return Ok(Vec::new());
    }
    
    let mut printers = Vec::new();
    
    // Try to parse as array first, then as single object
    if let Ok(ps_printers) = serde_json::from_str::<Vec<serde_json::Value>>(raw_output) {
        for printer in ps_printers {
            if let Some(name) = printer["Name"].as_str() {
                let port_name = printer["PortName"].as_str().unwrap_or("");
                let share_name = printer["ShareName"].as_str();
                
                printers.push(NetworkPrinter {
                    name: share_name.unwrap_or(name).to_string(),
                    ip_address: extract_ip_from_port(port_name).unwrap_or_else(|| "unknown".to_string()),
                    port: 9100, // Default IPP port
                    model: printer["DriverName"].as_str().map(|s| s.to_string()),
                    status: "Network (PowerShell)".to_string(),
                });
            }
        }
    } else if let Ok(printer) = serde_json::from_str::<serde_json::Value>(raw_output) {
        if let Some(name) = printer["Name"].as_str() {
            let port_name = printer["PortName"].as_str().unwrap_or("");
            let share_name = printer["ShareName"].as_str();
            
            printers.push(NetworkPrinter {
                name: share_name.unwrap_or(name).to_string(),
                ip_address: extract_ip_from_port(port_name).unwrap_or_else(|| "unknown".to_string()),
                port: 9100,
                model: printer["DriverName"].as_str().map(|s| s.to_string()),
                status: "Network (PowerShell)".to_string(),
            });
        }
    }
    
    Ok(printers)
}

fn discover_printers_port_scan() -> Result<Vec<NetworkPrinter>, String> {
    // Use ARP-based discovery instead of full network scan for better performance
    match discover_printers_arp_scan() {
        Ok(arp_printers) => {
            if !arp_printers.is_empty() {
                return Ok(arp_printers);
            }
        }
        Err(e) => eprintln!("ARP scan failed: {}", e),
    }
    
    // Fallback to limited network scan (only first 20 IPs)
    let network_range = get_local_network_range()?;
    let common_ports = vec![9100, 515, 631]; // IPP, LPR, CUPS
    
    let found_printers = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    
    // Limit to first 20 IPs to avoid long delays
    let limited_range: Vec<_> = network_range.into_iter().take(20).collect();
    eprintln!("Scanning {} IP addresses for printers...", limited_range.len());
    
    for ip in limited_range {
        for &port in &common_ports {
            let found_printers = Arc::clone(&found_printers);
            let handle = thread::spawn(move || {
                if is_printer_port_open(ip, port) {
                    let printer = NetworkPrinter {
                        name: format!("Network Printer at {}", ip),
                        ip_address: ip.to_string(),
                        port,
                        model: None,
                        status: "Port Scan Discovery".to_string(),
                    };
                    found_printers.lock().unwrap().push(printer);
                }
            });
            handles.push(handle);
        }
    }
    
    // Wait for all threads to complete
    for handle in handles {
        let _ = handle.join();
    }
    
    let printers = found_printers.lock().unwrap().clone();
    Ok(printers)
}

fn discover_printers_arp_scan() -> Result<Vec<NetworkPrinter>, String> {
    // Use ARP table to find devices on the local network
    let output = Command::new("arp")
        .arg("-a")
        .output()
        .map_err(|e| format!("Failed to execute arp command: {}", e))?;
    
    if !output.status.success() {
        return Err("ARP command failed".to_string());
    }
    
    let raw_output = String::from_utf8_lossy(&output.stdout);
    let mut potential_printers = Vec::new();
    
    // Parse ARP output to get IP addresses
    for line in raw_output.lines() {
        if let Some(ip_str) = extract_ip_from_arp_line(line) {
            if let Ok(ip) = Ipv4Addr::from_str(&ip_str) {
                // Skip common router IPs and broadcast addresses
                if !is_likely_router_ip(&ip) {
                    potential_printers.push(ip);
                }
            }
        }
    }
    
    let found_printers = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();
    
    // Check each IP for printer ports
    let common_ports = vec![9100, 515, 631]; // IPP, LPR, CUPS
    
    for ip in potential_printers {
        for &port in &common_ports {
            let found_printers = Arc::clone(&found_printers);
            let handle = thread::spawn(move || {
                if is_printer_port_open(ip, port) {
                    // Try to get printer info via SNMP or HTTP if possible
                    let printer_info = get_printer_info_via_snmp(ip).unwrap_or_else(|| {
                        format!("Network Printer at {}", ip)
                    });
                    
                    let printer = NetworkPrinter {
                        name: printer_info,
                        ip_address: ip.to_string(),
                        port,
                        model: None,
                        status: "ARP Discovery".to_string(),
                    };
                    found_printers.lock().unwrap().push(printer);
                }
            });
            handles.push(handle);
        }
    }
    
    // Wait for all threads to complete with timeout
    for handle in handles {
        let _ = handle.join();
    }
    
    let printers = found_printers.lock().unwrap().clone();
    Ok(printers)
}

fn extract_ip_from_arp_line(line: &str) -> Option<String> {
    // ARP output format varies, but typically contains IP addresses in parentheses or as first element
    if let Some(start) = line.find('(') {
        if let Some(end) = line.find(')') {
            let ip_str = &line[start + 1..end];
            if Ipv4Addr::from_str(ip_str).is_ok() {
                return Some(ip_str.to_string());
            }
        }
    }
    
    // Alternative format: IP as first element
    let parts: Vec<&str> = line.split_whitespace().collect();
    if let Some(first) = parts.first() {
        if Ipv4Addr::from_str(first).is_ok() {
            return Some(first.to_string());
        }
    }
    
    None
}

fn is_likely_router_ip(ip: &Ipv4Addr) -> bool {
    let octets = ip.octets();
    // Common router IPs end in .1, .254, or .255
    matches!(octets[3], 1 | 254 | 255)
}

fn get_printer_info_via_snmp(ip: Ipv4Addr) -> Option<String> {
    // Simple HTTP request to get printer info (many printers have web interfaces)
    // This is a simplified approach - in a real implementation, you might use SNMP
    
    // Try to connect to common printer web interface ports
    if is_printer_port_open(ip, 80) || is_printer_port_open(ip, 443) {
        return Some(format!("Network Printer at {}", ip));
    }
    
    None
}

fn extract_computer_name(line: &str) -> Option<String> {
    if let Some(start) = line.find("\\\\") {
        let name_part = &line[start + 2..];
        if let Some(end) = name_part.find(' ') {
            return Some(name_part[..end].to_string());
        }
        return Some(name_part.trim().to_string());
    }
    None
}

fn resolve_hostname(hostname: &str) -> Result<String, String> {
    let output = Command::new("nslookup")
        .arg(hostname)
        .output()
        .map_err(|e| format!("Failed to resolve hostname: {}", e))?;
    
    let raw_output = String::from_utf8_lossy(&output.stdout);
    
    for line in raw_output.lines() {
        if line.contains("Address:") && !line.contains("#") {
            if let Some(ip_start) = line.find("Address:") {
                let ip_part = line[ip_start + 8..].trim();
                if Ipv4Addr::from_str(ip_part).is_ok() {
                    return Ok(ip_part.to_string());
                }
            }
        }
    }
    
    Err("Could not resolve hostname".to_string())
}

fn extract_ip_from_port(port_name: &str) -> Option<String> {
    // Extract IP address from port names like "IP_192.168.1.100" or "192.168.1.100"
    if let Some(ip_start) = port_name.find("IP_") {
        let ip_part = &port_name[ip_start + 3..];
        if Ipv4Addr::from_str(ip_part).is_ok() {
            return Some(ip_part.to_string());
        }
    }
    
    // Try to parse the port name directly as an IP
    if Ipv4Addr::from_str(port_name).is_ok() {
        return Some(port_name.to_string());
    }
    
    None
}

fn get_local_network_range() -> Result<Vec<Ipv4Addr>, String> {
    let output = Command::new("ipconfig")
        .output()
        .map_err(|e| format!("Failed to get IP config: {}", e))?;
    
    let raw_output = String::from_utf8_lossy(&output.stdout);
    
    let mut local_ip = None;
    for line in raw_output.lines() {
        if line.contains("IPv4 Address") {
            if let Some(ip_start) = line.find(":") {
                let ip_part = line[ip_start + 1..].trim();
                if let Ok(ip) = Ipv4Addr::from_str(ip_part) {
                    // Skip loopback addresses
                    if !ip.is_loopback() {
                        local_ip = Some(ip);
                        break;
                    }
                }
            }
        }
    }
    
    if let Some(ip) = local_ip {
        let octets = ip.octets();
        let mut range = Vec::new();
        
        // Generate network range (assuming /24 subnet)
        for i in 1..255 {
            range.push(Ipv4Addr::new(octets[0], octets[1], octets[2], i));
        }
        
        Ok(range)
    } else {
        Err("Could not determine local IP address".to_string())
    }
}

fn is_printer_port_open(ip: Ipv4Addr, port: u16) -> bool {
    match TcpStream::connect_timeout(&(ip, port).into(), Duration::from_millis(100)) {
        Ok(_) => true,
        Err(_) => false,
    }
}

#[tauri::command]
fn list_devices() -> Result<Vec<String>, String> {
    eprintln!("Attempting device command for {}", std::env::consts::OS);
    let (cmd, args, parser): (&str, Vec<&str>, fn(&str) -> Vec<String>) = match std::env::consts::OS {
        "windows" => (
            "powershell",
            vec!["-Command", "Get-PnpDevice -Class 'Keyboard','Mouse' | Select-Object -Property Name | ConvertTo-Json -Compress"],
            |output| {
                match serde_json::from_str::<Vec<Device>>(output) {
                    Ok(devices) => devices.into_iter().map(|d| d.Name).collect(),
                    Err(_) => match serde_json::from_str::<Device>(output) {
                        Ok(device) => vec![device.Name],
                        Err(_) => vec![],
                    },
                }
            },
        ),
        "macos" => (
            "system_profiler",
            vec!["SPUSBDataType"],
            |output| {
                output
                    .lines()
                    .filter(|line| line.contains("Keyboard") || line.contains("Mouse"))
                    .map(|line| line.trim().to_string())
                    .collect()
            },
        ),
        "linux" => (
            "lsusb",
            vec![],
            |output| {
                output
                    .lines()
                    .filter(|line| line.to_lowercase().contains("keyboard") || line.to_lowercase().contains("mouse"))
                    .map(|line| line.trim().to_string())
                    .collect()
            },
        ),
        _ => return Err("Unsupported operating system".to_string()),
    };

    let output = std::process::Command::new(cmd)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to execute command {}: {}", cmd, e))?;

    if !output.status.success() {
        return Err(format!("Command {} failed: {}", cmd, String::from_utf8_lossy(&output.stderr)));
    }

    let raw_output = String::from_utf8_lossy(&output.stdout);
    eprintln!("Raw device command output: {}", raw_output);

    let device_names = parser(&raw_output);
    eprintln!("Parsed device names: {:?}", device_names);
    Ok(device_names)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            process_text, 
            list_printers, 
            list_all_printers, 
            list_devices
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}