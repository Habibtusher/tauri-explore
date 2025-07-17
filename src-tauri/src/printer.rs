
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{Ipv4Addr, TcpStream};
use std::process::Command;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use snmp::{SyncSession, Value};
use reqwest::blocking::Client;
use scraper::Html;
use scraper::Selector;
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
pub fn list_all_printers() -> Result<Vec<NetworkPrinter>, String> {
    let mut all_printers = Vec::new();
    
    // Get locally installed printers first
    match List_local_printers() {
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
            for v in &network_printers {
                println!("new printer {}", v.name);
            }
            all_printers.extend(network_printers);
        }
        Err(e) => eprintln!("Failed to discover network printers: {}", e),
    }
    
    Ok(all_printers)
}

pub fn query_printer_snmp(ip: &str) -> Option<(String, Option<String>)> {
    // Validate IP address
    if Ipv4Addr::from_str(ip).is_err() {
        eprintln!("Invalid IP address: {}", ip);
        return None;
    }

    let community = b"public";
    let timeout = std::time::Duration::from_secs(1);
    let target = format!("{}:161", ip);

    // Attempt SNMP query
    match SyncSession::new(&target, community, Some(timeout), 0) {
        Ok(mut session) => {
            // OIDs for printer name and model
            let name_oid = &[1, 3, 6, 1, 2, 1, 1, 5, 0]; // sysName
            let model_oids = [
                &[1, 3, 6, 1, 2, 1, 25, 3, 2, 1, 3, 1], // hrDeviceDescr
                &[1, 3, 6, 1, 2, 1, 43, 5, 1, 1, 16, 1], // prtGeneralPrinterName
            ];

            // Query printer name
            let name = session.get(name_oid).ok().and_then(|pdu| {
                pdu.varbinds.into_iter().next().and_then(|(_, val)| {
                    if let Value::OctetString(bytes) = val {
                        Some(String::from_utf8_lossy(&bytes).to_string())
                    } else {
                        None
                    }
                })
            });

            // Try multiple OIDs for model
            let mut model = None;
            for &oid in model_oids.iter() {
                if let Ok(pdu) = session.get(oid) {
                    model = pdu.varbinds.into_iter().next().and_then(|(_, val)| {
                        if let Value::OctetString(bytes) = val {
                            Some(String::from_utf8_lossy(&bytes).to_string())
                        } else {
                            None
                        }
                    });
                    if model.is_some() {
                        break;
                    }
                }
            }

            // Fallback to HTTP if model is not found
            let model = model.or_else(|| query_printer_http(ip));

            if let Some(name) = name {
                return Some((name, model));
            } else {
                eprintln!("Failed to retrieve printer name via SNMP for IP: {}", ip);
            }
        }
        Err(e) => {
            eprintln!("SNMP session failed for IP {}: {}", ip, e);
        }
    }

    // Final fallback to HTTP if SNMP fails entirely
    query_printer_http(ip).map(|model| (model.clone(), Some(model)))
}

/// Fallback function to query printer information via HTTP.
///
/// Attempts to scrape the printer's web interface for model information.
fn query_printer_http(ip: &str) -> Option<String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;
    
    let url = format!("http://{}", ip);
    match client.get(&url).send() {
        Ok(response) => {
            if response.status().is_success() {
                let body = response.text().ok()?;
                let document = Html::parse_document(&body);
                
                // Try common selectors for printer model
                let selectors = vec![
                    Selector::parse("title").unwrap(),
                    Selector::parse("h1").unwrap(),
                    Selector::parse("meta[name='description']").unwrap(),
                ];

                for selector in selectors {
                    if let Some(element) = document.select(&selector).next() {
                        let text = element.text().collect::<Vec<_>>().join(" ");
                        if !text.is_empty() {
                            return Some(text);
                        }
                    }
                }
            }
            None
        }
        Err(e) => {
            eprintln!("HTTP query failed for IP {}: {}", ip, e);
            None
        }
    }
}
pub fn List_local_printers() -> Result<Vec<String>, String> {
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

pub fn discover_network_printers() -> Result<Vec<NetworkPrinter>, String> {
    let mut network_printers = Vec::new();
    println!("Start discrovering..");
    // Method 1: Use Windows NET VIEW command
    if let Ok(net_view_printers) = discover_printers_net_view() {
        network_printers.extend(net_view_printers);
    } else {
        println!("discover_printers_net_view not Ok");
    }
    
    // Method 2: Use PowerShell WMI to find network printers
    if let Ok(wmi_printers) = discover_printers_wmi() {
        network_printers.extend(wmi_printers);
    }else {
        println!("discover_printers_wmi not Ok");
    }
    
    // Method 3: Port scan common printer ports on local network
    if let Ok(scanned_printers) = discover_printers_port_scan() {
        network_printers.extend(scanned_printers);
    }else {
        println!("discover_printers_port_scan not Ok");
    }
    
    // Remove duplicates based on IP address
    let mut unique_printers = Vec::new();
    let mut seen_ips = HashSet::new();
    
    for printer in network_printers {
        if seen_ips.insert(printer.ip_address.clone()) {
            unique_printers.push(printer);
        }
    }
    
    Ok(unique_printers)
}

pub fn discover_printers_net_view() -> Result<Vec<NetworkPrinter>, String> {
    let output = Command::new("net")
        .arg("view")
        .output()
        .map_err(|e| format!("Failed to execute net view: {}", e))?;
    
    if !output.status.success() {
        println!("Printing scaner not sucess returning empty");
        return Ok(Vec::new());
    }
    
    let raw_output = String::from_utf8_lossy(&output.stdout);
    let mut printers = Vec::new();
    println!("discover_printers_net_view{}", raw_output);
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

pub fn discover_printers_wmi() -> Result<Vec<NetworkPrinter>, String> {
    println!("call discover_printers_wmi");
    let output = Command::new("powershell")
        .arg("-Command")
        .arg("Get-WmiObject -Class Win32_Printer | Where-Object {$_.Network -eq $true} | Select-Object Name, PortName, DriverName | ConvertTo-Json -Compress")
        .output()
        .map_err(|e| format!("Failed to execute PowerShell WMI: {}", e))?;
    
    if !output.status.success() {
        println!("discover_printers_wmi not success return empty");
        return Ok(Vec::new());
    }
    
    let raw_output = String::from_utf8_lossy(&output.stdout);
    if raw_output.trim().is_empty() {
        println!("Raw output is empty");
        return Ok(Vec::new());
    }
    
    let mut printers = Vec::new();
    println!("discover_printers_wmi {}", raw_output);
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

pub fn discover_printers_port_scan() -> Result<Vec<NetworkPrinter>, String> {
    println!("discover_printers_port_scan call....");
    let network_range = get_local_network_range()?;
    let common_ports = vec![9100, 515, 631]; // IPP, LPR, CUPS

    let found_printers = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    // Limit concurrent threads to avoid overwhelming the network
    let chunk_size = 10;
    for chunk in network_range.chunks(chunk_size) {
        for &ip in chunk {
            for &port in &common_ports {
                let found_printers = Arc::clone(&found_printers);
                let ip_clone = ip.clone();
                let handle = thread::spawn(move || {
                    if is_printer_port_open(ip_clone, port) {
                        let ip_str = ip_clone.to_string();
                        println!("Port is open: IP: {} PORT: {}", ip_str, port);

                        let (name, model) = query_printer_snmp(&ip_str)
                            .unwrap_or_else(|| (format!("Network Printer at {}", ip_str), None));

                        let printer = NetworkPrinter {
                            name,
                            ip_address: ip_str,
                            port,
                            model,
                            status: "Discovered (SNMP)".to_string(),
                        };
                        found_printers.lock().unwrap().push(printer);
                    }
                });
                handles.push(handle);
            }
        }

        // Wait for this chunk to complete before starting the next
        for handle in handles.drain(..) {
            let _ = handle.join();
        }
    }

    let printers = found_printers.lock().unwrap().clone();
    Ok(printers)
}


pub fn extract_computer_name(line: &str) -> Option<String> {
    if let Some(start) = line.find("\\\\") {
        let name_part = &line[start + 2..];
        if let Some(end) = name_part.find(' ') {
            return Some(name_part[..end].to_string());
        }
        return Some(name_part.trim().to_string());
    }
    None
}

pub fn resolve_hostname(hostname: &str) -> Result<String, String> {
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

pub fn extract_ip_from_port(port_name: &str) -> Option<String> {
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

pub fn get_local_network_range() -> Result<Vec<Ipv4Addr>, String> {
    let output = Command::new("ipconfig")
        .output()
        .map_err(|e| format!("Failed to get IP config: {}", e))?;
    
    let raw_output = String::from_utf8_lossy(&output.stdout);

    println!("Network range output {}", raw_output);
    
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


