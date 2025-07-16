// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Deserialize;

#[derive(Deserialize)]
struct Printer {
    Name: String,
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
    let output = std::process::Command::new("powershell")
        .arg("-Command")
        .arg("Get-Printer | Select-Object -Property Name | ConvertTo-Json -Compress")
        .output()
        .map_err(|e| format!("Failed to execute PowerShell command: {}", e))?;

    if !output.status.success() {
        return Err(format!("PowerShell command failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let raw_output = String::from_utf8_lossy(&output.stdout);
    eprintln!("Raw PowerShell output: {}", raw_output);

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
fn list_devices() -> Result<Vec<String>, String> {
    let (cmd, args, parser): (&str, Vec<&str>, fn(&str) -> Vec<String>) = match std::env::consts::OS {
        "windows" => (
            "powershell",
            vec!["-Command", "Get-PnpDevice -Class 'Keyboard','Mouse' | Select-Object -Property Name | ConvertTo-Json -Compress"],
            |output| {
                #[derive(Deserialize)]
                struct Device {
                    Name: String,
                }
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
        .invoke_handler(tauri::generate_handler![process_text, list_printers, list_devices])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}