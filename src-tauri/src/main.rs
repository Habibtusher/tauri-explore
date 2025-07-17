
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize};
use app::printer::*;

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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            process_text, 
            list_printers, 
            List_all_printers, 

        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}