use actix_web::{web, App, HttpServer, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::str;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
struct CreateWalletResponse {
    address: String,
    wallet_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct WalletInfo {
    address: String,
    balance: String,
    wallet_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BalanceResponse {
    success : bool,
    balance: String,
    wallet_id : String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SendRequest {
    to_address: String,
    amount: String,
    memo: Option<String>,
    wallet_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SendResponse {
    success: bool,
    transaction_id: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}


#[derive(Debug, Serialize, Deserialize)]
struct WalletSummary {
    wallet_id: String,
    address: String,
}

// In-memory storage for wallets
struct AppState {
    wallets: Mutex<HashMap<String, WalletData>>,
}

#[derive(Clone)]
struct WalletData {
    address: String,
    data_dir: String,
}

impl AppState {
    fn new() -> Self {
        AppState {
            wallets: Mutex::new(HashMap::new()),
        }
    }
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Zingo API Server is running!",
        "version": "1.0.0"
    }))
}

async fn create_wallet(data: web::Data<AppState>) -> impl Responder {
    let wallet_id = Uuid::new_v4().to_string();
    
    // Create in project folder: ./wallets/{wallet_id}/
    let data_dir = format!("./wallets/{}", wallet_id);
    
    println!("📁 Creating directory: {}", data_dir);
    
    // Create the directory
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        eprintln!("❌ Failed to create directory: {}", e);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error: format!("Failed to create wallet directory: {}", e),
        });
    }

    println!("✅ Directory created successfully at: {}", data_dir);
    
    // Use zingo-cli with the relative path
    let zingo_cli_path = "./target/release/zingo-cli";
    
    let output = match Command::new(zingo_cli_path)
        .args(&[
            "--data-dir", &data_dir
        ])
        .output()
    {
        Ok(output) => {
            println!("✅ Command executed successfully");
            println!("Status: {}", output.status);
            println!("Stdout: {}", String::from_utf8_lossy(&output.stdout));
            output
        },
        Err(e) => {
            eprintln!("❌ Failed to execute zingo-cli: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                success: false,
                error: format!("Failed to initialize wallet: {}", e),
            });
        }
    };

   


    if !output.status.success() {
        let error = str::from_utf8(&output.stderr).unwrap_or("Unknown error");
        eprintln!("zingo-cli init error: {}", error);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error: error.to_string(),
        });
    }

    
    // Get the address for this wallet
    let address_output = match Command::new("./target/release/zingo-cli")
        .args(&[
            "--data-dir", &data_dir,
            "addresses",
        ])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            eprintln!("Failed to get address: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                success: false,
                error: "Failed to get wallet address".to_string(),
            });
        }
    };

    let address = if address_output.status.success() {
    let output_str = str::from_utf8(&address_output.stdout).unwrap_or("").trim();
    
    // Look for the encoded_address field
    if let Some(addr_start) = output_str.find("\"encoded_address\": \"") {
        let addr_start = addr_start + "\"encoded_address\": \"".len();
        if let Some(addr_end) = output_str[addr_start..].find('\"') {
            output_str[addr_start..addr_start + addr_end].to_string()
        } else {
            "Address format error".to_string()
        }
        } else {
            "Address not found in output".to_string()
        }
    } else {
        "Address not available".to_string()
    };
    // Store wallet data
    let wallet_data = WalletData {
        address: address.clone(),
        data_dir: data_dir.clone(),
    };
    {
        let mut wallets = data.wallets.lock().unwrap();
        wallets.insert(wallet_id.clone(), wallet_data);
    }
    HttpResponse::Ok().json(CreateWalletResponse {
        address,
        wallet_id,
    })
}



// Fixed path parameter syntax
async fn get_wallet_info(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let wallet_id = path.into_inner();
    
    let wallets = data.wallets.lock().unwrap();
    
    let wallet_data = match wallets.get(&wallet_id) {
        Some(data) => data,
        None => {
            return HttpResponse::NotFound().json(ErrorResponse {
                success: false,
                error: "Wallet not found".to_string(),
            });
        }
    };

    // Get balance for this specific wallet
    let output = match Command::new("/Users/user/Documents/zingolib/target/release/zingo-cli")
        .args(&[
            "--data-dir", &wallet_data.data_dir,
            "balance",
        ])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            eprintln!("Failed to execute zingo-cli: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                success: false,
                error: "Failed to get wallet balance".to_string(),
            });
        }
    };

    if output.status.success() {
        let balance = str::from_utf8(&output.stdout).unwrap_or("0").trim().to_string();
        
        HttpResponse::Ok().json(WalletInfo {
            address: wallet_data.address.clone(),
            balance,
            wallet_id,
        })
    } else {
        let error = str::from_utf8(&output.stderr).unwrap_or("Unknown error");
        HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error: error.to_string(),
        })
    }
}

// Fixed path parameter syntax
async fn get_balance(
    _data: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let wallet_id = path.into_inner();
    
    let data_dir = format!("./wallets/{}", wallet_id);
    let output = match Command::new("./target/release/zingo-cli")
        .args(&[
            "--data-dir", &data_dir,
            "balance",
        ])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            eprintln!("Failed to execute zingo-cli: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                success: false,
                error: "Failed to get balance".to_string(),
            });
        }
    };

    if output.status.success() {
        let output_str = str::from_utf8(&output.stdout).unwrap_or("").trim();
        
        // Extract confirmed_orchard_balance
        let balance_value = if let Some(start) = output_str.find("confirmed_orchard_balance: ") {
            let start = start + "confirmed_orchard_balance: ".len();
            if let Some(end) = output_str[start..].find('\n') {
                output_str[start..start + end].trim().to_string()
            } else {
                "0".to_string()
            }
        } else {
            "0".to_string()
        };

        HttpResponse::Ok().json(BalanceResponse {
            success : true,
            balance: balance_value,
            wallet_id : wallet_id
        })
    } else {
        let error = str::from_utf8(&output.stderr).unwrap_or("Unknown error");
        HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error: error.to_string(),
        })
    }
}

async fn send_zec(
    data: web::Data<AppState>,
    send_req: web::Json<SendRequest>,
) -> impl Responder {
    let wallets = data.wallets.lock().unwrap();
    
    // Use default wallet if none specified, or get specific wallet
    let wallet_data = if let Some(wallet_id) = &send_req.wallet_id {
        match wallets.get(wallet_id) {
            Some(data) => data,
            None => {
                return HttpResponse::NotFound().json(ErrorResponse {
                    success: false,
                    error: "Wallet not found".to_string(),
                });
            }
        }
    } else {
        // Use first wallet as default (for simplicity)
        match wallets.values().next() {
            Some(data) => data,
            None => {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    success: false,
                    error: "No wallets available".to_string(),
                });
            }
        }
    };

    let mut args = vec![
        "--data-dir".to_string(),
        wallet_data.data_dir.clone(),
        "send".to_string(),
        send_req.to_address.clone(),
        send_req.amount.clone(),
    ];

    if let Some(memo) = &send_req.memo {
        args.push("--memo".to_string());
        args.push(memo.clone());
    }

    let output = match Command::new("/Users/user/Documents/zingolib/target/release/zingo-cli")
        .args(&args)
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            eprintln!("Failed to execute zingo-cli: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                success: false,
                error: "Failed to send ZEC".to_string(),
            });
        }
    };

    if output.status.success() {
        let tx_id = str::from_utf8(&output.stdout).unwrap_or("").trim().to_string();
        
        HttpResponse::Ok().json(SendResponse {
            success: true,
            transaction_id: tx_id,
            message: "ZEC sent successfully".to_string(),
        })
    } else {
        let error = str::from_utf8(&output.stderr).unwrap_or("Unknown error");
        HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error: error.to_string(),
        })
    }
}

async fn list_wallets(data: web::Data<AppState>) -> impl Responder {
    let wallets = data.wallets.lock().unwrap();
    let wallet_list: Vec<WalletSummary> = wallets.iter()
        .map(|(id, data)| WalletSummary {
            wallet_id: id.clone(),
            address: data.address.clone(),
        })
        .collect();
    
    HttpResponse::Ok().json(wallet_list)
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting Zingo API Server on http://localhost:8080");
    
    let app_state = web::Data::new(AppState::new());
    
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/health", web::get().to(health_check))
            .route("/api/wallet/create", web::post().to(create_wallet))
            .route("/api/wallet/list", web::get().to(list_wallets))
            .route("/api/wallet/{wallet_id}/info", web::get().to(get_wallet_info))
            .route("/api/wallet/{wallet_id}/balance", web::get().to(get_balance))
            .route("/api/wallet/send", web::post().to(send_zec))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}